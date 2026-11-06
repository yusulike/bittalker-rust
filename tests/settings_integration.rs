use bittalker::core::interval::{Direction, IntervalTracker};
use bittalker::i18n::{announcement_text, voice_test_text};
use bittalker::settings::{lang_code, Settings, SettingsManager};
use bittalker::ui::settings_dialog::SettingsDialog;

#[test]
fn test_settings_dialog_flow() {
    let temp_dir = std::env::temp_dir();
    let file = temp_dir.join(format!("test_flow_{}.json", std::process::id()));

    let mut manager = SettingsManager::new(&file);
    assert_eq!(manager.get().interval, 50);
    assert_eq!(manager.get().voice, "F1");
    assert_eq!(manager.get().language, "Korean");
    assert!(manager.get().always_on_top);
    assert!(!manager.get().ticker_mode);

    // 1. Open dialog
    let mut dialog = SettingsDialog::open(manager.get(), None);
    assert!(dialog.open);

    // 2. User edits draft in dialog
    dialog.interval = 500;
    dialog.voice = "M3".to_string();
    dialog.language = "English".to_string();
    dialog.always_on_top = false;
    dialog.ticker_mode = true;

    // 3. User tests voice in dialog
    let mut test_invoked = false;
    let mut on_test_voice = |voice: &str, lang: &str| {
        test_invoked = true;
        assert_eq!(voice, "M3");
        assert_eq!(lang, "English");
        let text = voice_test_text(lang, Some(95000.0));
        assert!(text.contains("Current Bitcoin price is 95000 dollars"));
    };
    on_test_voice(&dialog.voice, &dialog.language);
    assert!(test_invoked);

    // 4. Before pressing OK, disk & manager still retain original
    assert_eq!(manager.get().interval, 50);
    assert_eq!(manager.get().language, "Korean");

    // 5. User clicks OK -> draft applied to manager and tracker
    let new_settings = Settings {
        interval: dialog.interval,
        voice: dialog.voice.clone(),
        language: dialog.language.clone(),
        always_on_top: dialog.always_on_top,
        muted: manager.get().muted,
        ticker_mode: dialog.ticker_mode,
        window_pos: None,
        weather_location: dialog.weather_location.clone(),
    };
    manager.update(new_settings);

    let tracker = IntervalTracker::new(manager.get().interval as f64);
    assert_eq!(tracker.interval, 500.0);

    // 6. Reload from disk to verify persistence
    let reloaded = SettingsManager::new(&file);
    assert_eq!(reloaded.get().interval, 500);
    assert_eq!(reloaded.get().voice, "M3");
    assert_eq!(reloaded.get().language, "English");
    assert!(!reloaded.get().always_on_top);
    assert!(reloaded.get().ticker_mode);

    // 7. Verify i18n announcement generation matches new settings
    let ann = announcement_text(
        &reloaded.get().language,
        95500,
        Direction::Up,
    );
    assert_eq!(ann, "Bitcoin passed 95500 dollars.");
    assert_eq!(lang_code(&reloaded.get().language), "en");

    let _ = std::fs::remove_file(file);
}
