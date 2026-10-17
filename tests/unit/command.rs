//! Модульные тесты src/command.rs (подключены там через `#[path]`, видят приватное).

use super::*;

#[test]
fn names_roundtrip() {
    for c in Command::ALL {
        assert_eq!(c.to_string().parse::<Command>(), Ok(*c), "{c}");
    }
    assert_eq!("space:3".parse(), Ok(Command::Space(3)));
    assert_eq!("goto:9".parse(), Ok(Command::Goto(9)));
    assert_eq!(Command::Space(2).to_string(), "space:2");
}

#[test]
fn rejects_unknown_and_out_of_range() {
    for bad in ["", "Palette", "orbit ", "space:0", "space:10", "goto:", "goto:x", "nope"] {
        assert!(bad.parse::<Command>().is_err(), "{bad}");
    }
}

#[test]
fn deserializes_from_json_string() {
    assert_eq!(serde_json::from_str::<Command>(r#""split_swap""#).unwrap(), Command::SplitSwap);
    assert!(serde_json::from_str::<Command>(r#""unknown""#).is_err());
}

#[test]
fn only_switching_repeats() {
    assert!(Command::Next.repeats() && Command::PrevSeq.repeats());
    assert!(!Command::Close.repeats() && !Command::Orbit.repeats());
}
