fn main() {
    let args = "/bin/hello &";
    let mut is_background = false;
    let clean_args = if args.trim_end().ends_with('&') {
        is_background = true;
        args.trim_end().trim_end_matches('&').trim()
    } else {
        args.trim()
    };
    println!("is_background: {}, clean_args: '{}'", is_background, clean_args);
}
