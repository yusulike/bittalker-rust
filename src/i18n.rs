//! Localized announcement text builders.
//! Port of the message templates in `src/ui/main_window.py`
//! (`on_interval_crossed` / `run_voice_test`).

use crate::core::interval::Direction;
use crate::korean_numbers::number_to_korean;

/// Announcement played when a price boundary is crossed.
pub fn announcement_text(language: &str, price_int: i64, direction: Direction) -> String {
    let up = direction == Direction::Up;
    match language {
        "Korean" => {
            // Convert number to Korean text for proper pronunciation
            let price_korean = number_to_korean(price_int);
            if up {
                format!("{price_korean}달러를 돌파했습니다.")
            } else {
                format!("{price_korean}달러가 깨어졌습니다.")
            }
        }
        "English" => {
            if up {
                format!("Bitcoin passed {price_int} dollars.")
            } else {
                format!("Bitcoin dropped below {price_int} dollars.")
            }
        }
        "Spanish" => {
            if up {
                format!("Bitcoin superó los {price_int} dólares.")
            } else {
                format!("Bitcoin cayó por debajo de los {price_int} dólares.")
            }
        }
        "Portuguese" => {
            if up {
                format!("O Bitcoin ultrapassou {price_int} dólares.")
            } else {
                format!("O Bitcoin caiu abaixo de {price_int} dólares.")
            }
        }
        "French" => {
            if up {
                format!("Le Bitcoin a dépassé {price_int} dollars.")
            } else {
                format!("Le Bitcoin est tombé sous {price_int} dollars.")
            }
        }
        _ => {
            // Same fallback as the original: unknown language behaves as Korean
            let price_korean = number_to_korean(price_int);
            if up {
                format!("{price_korean}달러를 돌파했습니다.")
            } else {
                format!("{price_korean}달러가 깨어졌습니다.")
            }
        }
    }
}

/// Text spoken by the "Test Voice" button; uses the last known price.
pub fn voice_test_text(language: &str, price: Option<f64>) -> String {
    let price_val = price.map(|p| p as i64).unwrap_or(0);
    match language {
        "Korean" => {
            if let Some(_) = price {
                let price_korean = number_to_korean(price_val);
                format!("현재 비트코인 가격은 {price_korean}달러입니다.")
            } else {
                "비트코인 가격 정보를 기다리고 있어요".to_string()
            }
        }
        "English" => match price {
            Some(_) => format!("Current Bitcoin price is {price_val} dollars."),
            None => "Waiting for Bitcoin price data.".to_string(),
        },
        "Spanish" => match price {
            Some(_) => format!("El precio actual de Bitcoin es {price_val} dólares."),
            None => "Esperando datos del precio de Bitcoin.".to_string(),
        },
        "Portuguese" => match price {
            Some(_) => format!("O preço atual do Bitcoin é {price_val} dólares."),
            None => "Aguardando dados de preço do Bitcoin.".to_string(),
        },
        "French" => match price {
            Some(_) => format!("Le prix actuel du Bitcoin est de {price_val} dollars."),
            None => "En attente des données sur le prix du Bitcoin.".to_string(),
        },
        _ => "비트코인 가격 정보를 기다리고 있어요".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korean_announcement() {
        assert_eq!(
            announcement_text("Korean", 95500, Direction::Up),
            "구만 오천오백달러를 돌파했습니다."
        );
        assert_eq!(
            announcement_text("Korean", 95500, Direction::Down),
            "구만 오천오백달러가 깨어졌습니다."
        );
    }

    #[test]
    fn english_announcement() {
        assert_eq!(
            announcement_text("English", 95500, Direction::Up),
            "Bitcoin passed 95500 dollars."
        );
        assert_eq!(
            announcement_text("English", 95500, Direction::Down),
            "Bitcoin dropped below 95500 dollars."
        );
    }

    #[test]
    fn voice_test() {
        assert_eq!(
            voice_test_text("English", Some(95_500.4)),
            "Current Bitcoin price is 95500 dollars."
        );
        assert_eq!(
            voice_test_text("English", None),
            "Waiting for Bitcoin price data."
        );
    }
}
