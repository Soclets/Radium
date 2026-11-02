// section oracle.tests.exit:
//! Checks the status a finished program reports to the system (D152).

fn status(source: &str) -> Option<u8> {
    match oracle::run_source(source) {
        oracle::Outcome::Runs { exit_status, .. } => Some(exit_status),
        _ => None,
    }
}

#[test]
fn a_number_from_fn_main_is_the_exit_status() {
    let source = "\
fn.main [] -> int[min: 0, max: 2]:
    return 2;
end fn.main
";
    assert_eq!(status(source), Some(2));
}

#[test]
fn a_void_program_that_finishes_reports_zero() {
    let source = "\
fn.main [] -> void:
    return void;
end fn.main
";
    assert_eq!(status(source), Some(0));
}
// end section oracle.tests.exit
