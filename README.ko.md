# BitTalker (Rust)

Python으로 구현되었던 실시간 바이낸스 비트코인(BTC/USDT) 시세 모니터 및 음성 알림 데스크톱 애플리케이션([yusulike/bittalker](https://github.com/yusulike/bittalker))의 **순수 Rust 포팅 버전**입니다.

[English Documentation (README.md)](README.md)

---

## 프로젝트 개요

BitTalker는 바이낸스 웹소켓을 통해 비트코인 체결 데이터를 실시간으로 수신하며, 사용자가 지정한 가격 구간(예: \$50, \$100 등)을 돌파할 때마다 온디바이스 ONNX Supertonic-3 TTS 엔진을 통해 자연스러운 음성으로 알림을 제공합니다.

기존 Python (PyQt6 + ONNX) 버전을 Rust로 완전히 재작성하여 다음과 같은 장점을 갖추었습니다:
- **Python 런타임 완전 배제**: Python 설치나 외부 인터프리터 의존성 없이 단일 네이티브 바이너리로 실행.
- **초경량 최적화**: OpenGL(`glow`) 렌더링 백엔드 및 CPU 전용 ONNX Runtime 동적 로딩을 채택하여 실행 파일 크기 약 13MB (불필요한 DirectML/WGPU 제거).
- **GUI 서브시스템 적용**: 릴리즈 실행 시 불필요한 콘솔(까만 터미널) 창 없이 깔끔하게 실행.
- **멀티모니터 창 위치 기억**: 재실행 시 보조 모니터(음수 좌표 영역 포함)를 비롯한 이전 창 위치를 그대로 복원.
- **상대 좌표 기반 설정창 오픈**: 메인 티커 창의 화면 위치를 감지하여 바로 근처(상/하단 중앙)에 독립 네이티브 설정창 표시.

---

## 주요 기능

- **실시간 바이낸스 웹소켓 수신**: `btcusdt@trade` 스트림을 연결하여 틱 단위 실시간 감시 및 끊김 시 자동 재연결.
- **레트로 데스크톱 디지털 시계 및 정각 차임벨**:
  - 플립 시계 스타일 카드 위젯 (시, 분, 초, AM/PM, 전체 날짜 표기).
  - 일반 모드 및 컴팩트 티커 모드 모두에서 실시간 초 단위 시계 표시.
  - 매시간 정각 자동 알림 오디오 차임벨 지원.
- **현재 지역 실시간 날씨 및 기온 정보**:
  - IP 기반 자동 감지 또는 설정 다이얼로그를 통한 직접 지역/도시명 설정.
  - 별도 API 키가 필요 없는 Open-Meteo 무료 Geocoding 및 기상 API 연동.
  - 설치된 폰트에 구애받지 않고 선명하게 그려지는 자체 벡터 날씨 아이콘(맑음, 흐림, 비, 눈, 번개, 안개 등).
  - 플립 시계 옆 3행 레이아웃(`AM/PM` / `날씨 아이콘` / `기온`) 및 마우스 호버 시 상세 정보 툴팁 제공.
- **임베디드 Supertonic-3 ONNX TTS**: 로컬 디바이스에서 5개 언어(한국어, 영어, 스페인어, 포르투갈어, 프랑스어) 및 10가지 음성 스타일(F1~F5, M1~M5) 합성.
- **가격 구간 돌파 알림**: 사용자가 지정한 가격 간격(예: \$50, \$100, \$500 등) 돌파 알림 및 진동 방지(anti-oscillation) 필터링.
- **프레임리스 egui 데스크톱 UI**:
  - 2가지 화면 모드: **일반 모드**(시계 + 가격 + 컨트롤) 및 **컴팩트 티커 모드**(단일 라인 바 형태).
  - 전체 화면 어디로나 마우스 드래그로 자유로운 이동 가능.
  - 재실행 시 다중 모니터 좌표 자동 기억 및 복원.
  - 항상 위(Always on Top) 설정 및 음소거 빠른 토글 버튼.
  - 메인 창 옆에 유기적으로 배치되는 독립 설정 다이얼로그.
- **내장 Asset 다운로더**: Python 없이 Rust 바이너리 자체에서 HuggingFace의 TTS 모델을 즉시 다운로드.

---

## 디렉터리 및 Asset 구조

배포 및 실행 시 **실행 파일(`bittalker.exe`) 바로 아래의 `assets/` 디렉터리가 기본 구조**입니다.

```text
[실행파일 디렉터리]/
├── bittalker.exe          # 메인 GUI 애플리케이션
├── onnxruntime.dll        # CPU 전용 ONNX Runtime 동적 라이브러리
├── assets/                # TTS 모델 및 음성 설정
│   ├── config.json
│   ├── onnx/
│   │   ├── duration_predictor.onnx
│   │   ├── text_encoder.onnx
│   │   ├── tts.json
│   │   ├── unicode_indexer.json
│   │   ├── vector_estimator.onnx
│   │   └── vocoder.onnx
│   └── voice_styles/
│       ├── F1.json ~ F5.json
│       └── M1.json ~ M5.json
├── cache/                 # TTS 생성 오디오 캐시 (자동 생성)
└── settings.json          # 사용자 설정 및 마지막 창 위치 (자동 생성)
```

---

## 시작하기

### 요구 사항
- [Rust 툴체인](https://rustup.rs/) (edition 2021)
- Windows 10/11 x64

### 1. Asset 모델 다운로드 (최초 1회)
HuggingFace에서 약 250MB 분량의 TTS 모델을 자동으로 다운로드합니다:

```bash
cargo run --bin download-assets
```
*(기본적으로 현재 빌드 타깃 디렉터리인 `target/debug/assets` 또는 `target/release/assets`에 저장됩니다.)*

메인 실행 바이너리 플래그로도 다운로드할 수 있습니다:
```bash
cargo run -- --download-assets
```

### 2. 애플리케이션 실행
```bash
cargo run
```

### 3. 단위 및 통합 테스트 실행
총 40개의 단위/통합 테스트를 검증합니다:
```bash
cargo test
```

---

## 릴리즈 빌드 및 배포 패키징

최적화된 릴리즈 바이너리 빌드:
```bash
cargo build --release
```

독립 포터블 배포용 ZIP 압축 파일 생성:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1
```

스크립트 실행 완료 시 `dist/bittalker-windows-x64.zip` 파일이 생성되며, 압축을 풀면 별도 설정 없이 바로 실행 가능한 포터블 패키지(`bittalker.exe` + `onnxruntime.dll` + `assets/`)가 구성됩니다.

---

## CLI 옵션 안내

| 옵션 | 설명 |
|---|---|
| *(옵션 없음)* | 메인 GUI 애플리케이션 실행 |
| `--download-assets [경로]` | 지정된 경로로 TTS 모델 파일 다운로드 (기본: `[실행파일_경로]/assets`) |
| `--check-assets` | 필요한 모든 모델 및 설정 파일의 무결성 검증 |
| `--open-settings` | 애플리케이션 시작과 동시에 설정 창 열기 |
| `--screenshot [경로]` | 일반 모드 UI 화면을 캡처하여 저장 후 자동 종료 |
| `--screenshot-ticker [경로]` | 컴팩트 티커 모드 UI 화면을 캡처하여 저장 후 자동 종료 |
| `--help`, `-h` | CLI 사용 안내문 출력 |

---

## References

- TTS Model: [supertonic](https://github.com/supertone-inc/supertonic)

---

## 라이센스 (License)

본 프로젝트는 MIT 라이센스 하에 배포됩니다. 자세한 내용은 [LICENSE](LICENSE) 파일을 참조하세요.
