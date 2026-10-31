// section oracle.tests.messages:
//! Checks the layout of the oracle's messages (D116–D119, D122): the place twice, the quoted line with a pointer,
//! the labelled lines and `because:` links.

/// Runs `source` and gives back its message as the command would print it.
fn rendered(source: &str) -> String {
    oracle::run_source(source).render("example.ra", source)
}

#[test]
fn a_failed_check_shows_what_why_when_and_because() {
    let source = "\
fn.main [] -> void:
    var $byte: int[min: 0, max: 255] = 200;
    $byte = $byte + 56;
    return void;
end fn.main
";
    let expected = "\
check failed: example.ra:3:13, line 3 column 13
 3 |     $byte = $byte + 56;
   |             ^^^^^^^^^^
 what: `$byte` holds int[min: 0, max: 255], and this value doesn't fit
 why:  every value stays within its bounds (D14)
 when: it would be 256
 because: example.ra:2:5, line 2 column 5
 2 |     var $byte: int[min: 0, max: 255] = 200;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     `$byte`'s bounds are set here
";
    assert_eq!(rendered(source), expected);
}

#[test]
fn exactly_one_fix_is_shown_in_place_of_what() {
    let source = "\
fn.main [] -> void:
    std::print(8 div.exact 2, newline);
    return void;
end fn.main
";
    let expected = "\
refused: example.ra:2:18, line 2 column 18
 2 |     std::print(8 div.exact 2, newline);
   |                  ^^^^^^^^^
 fix:  `div`
 why:  bare `div` is exact, and a default the language fixes is never spelled out (D37, D39)
";
    assert_eq!(rendered(source), expected);
}

#[test]
fn the_two_columns_differ_after_a_wide_character() {
    // `°` is 2 bytes but one grapheme cluster, so the tools' column is one higher (D117).
    let source = "\
fn.main [] -> void:
    std::print(\"max 125 °C: \", newln);
    return void;
end fn.main
";
    let message = rendered(source);
    assert!(
        message.starts_with("refused: example.ra:2:33, line 2 column 32\n"),
        "{message}"
    );
    assert!(
        message.contains("\n   |                                ^^^^^\n"),
        "{message}"
    );
}

#[test]
fn a_program_that_runs_has_no_message() {
    let source = "\
fn.main [] -> void:
    return void;
end fn.main
";
    assert_eq!(rendered(source), "");
}
// end section oracle.tests.messages
