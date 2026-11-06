// SPDX-License-Identifier: MIT OR Apache-2.0
//! Command-line parsing for the `tpt-codex-relay` binary.

use tpt_codex_sync::{
    DEFAULT_ADDR, DEFAULT_MAX_MESSAGE_BYTES, DEFAULT_MAX_OPS_PER_DOC, Limits, USAGE, parse_args,
};

fn parse(args: &[&str]) -> Result<tpt_codex_sync::Config, String> {
    parse_args(args.iter().map(|s| s.to_string()))
}

#[test]
fn no_arguments_gives_the_documented_defaults() {
    let config = parse(&[]).unwrap();
    assert_eq!(config.addr, DEFAULT_ADDR);
    assert_eq!(config.addr, "127.0.0.1:8787");
    assert_eq!(config.limits, Limits::default());
    assert_eq!(config.limits.max_message_bytes, DEFAULT_MAX_MESSAGE_BYTES);
    assert_eq!(config.limits.max_ops_per_doc, DEFAULT_MAX_OPS_PER_DOC);
}

#[test]
fn flags_take_a_separate_value_or_an_equals_sign() {
    let config = parse(&[
        "--addr",
        "0.0.0.0:9000",
        "--max-message-bytes",
        "4096",
        "--max-ops-per-doc=50",
    ])
    .unwrap();
    assert_eq!(config.addr, "0.0.0.0:9000");
    assert_eq!(config.limits.max_message_bytes, 4096);
    assert_eq!(config.limits.max_ops_per_doc, 50);

    let config = parse(&["--addr=127.0.0.1:0"]).unwrap();
    assert_eq!(config.addr, "127.0.0.1:0");
}

#[test]
fn a_positional_address_still_works() {
    let config = parse(&["127.0.0.1:9999"]).unwrap();
    assert_eq!(config.addr, "127.0.0.1:9999");
    assert_eq!(config.limits, Limits::default());
}

#[test]
fn a_second_positional_address_is_refused() {
    let err = parse(&["127.0.0.1:1", "127.0.0.1:2"]).unwrap_err();
    assert!(err.starts_with("more than one address"), "{err}");
}

#[test]
fn limits_must_be_positive_whole_numbers() {
    for bad in ["0", "-1", "abc", "1.5", ""] {
        let err = parse(&["--max-ops-per-doc", bad]).unwrap_err();
        assert!(err.contains("positive whole number"), "{bad}: {err}");
        let err = parse(&["--max-message-bytes", bad]).unwrap_err();
        assert!(err.contains("positive whole number"), "{bad}: {err}");
    }
}

#[test]
fn a_flag_without_a_value_is_refused() {
    let err = parse(&["--addr"]).unwrap_err();
    assert!(err.starts_with("--addr needs a value"), "{err}");
    let err = parse(&["--max-ops-per-doc"]).unwrap_err();
    assert!(err.starts_with("--max-ops-per-doc needs a value"), "{err}");
}

#[test]
fn an_empty_address_is_refused() {
    assert!(parse(&["--addr", "  "]).is_err());
    assert!(parse(&[""]).is_err());
}

#[test]
fn an_unknown_option_is_refused_with_usage() {
    let err = parse(&["--no-such-flag"]).unwrap_err();
    assert!(err.starts_with("unknown option --no-such-flag"), "{err}");
    assert!(err.contains("usage: tpt-codex-relay"), "{err}");
}

#[test]
fn help_returns_the_usage_text() {
    assert_eq!(parse(&["--help"]).unwrap_err(), USAGE);
    assert_eq!(parse(&["-h"]).unwrap_err(), USAGE);
}
