use super::YdotoolSyntax;

#[test]
fn legacy_ydotool_help_selects_key_names() {
    // Ubuntu's 0.1.8 package.
    let help = "Usage: key [--delay <ms>] [--key-delay <ms>] [--repeat <times>] \
                [--repeat-delay <ms>] <key sequence> ...\n\
                Each key sequence can be any number of modifiers and keys, separated by plus (+)";
    let syntax = YdotoolSyntax::from_key_help(help);
    assert_eq!(syntax, YdotoolSyntax::KeyNames);
    assert_eq!(syntax.paste_chord(), ["key", "ctrl+v"]);
}

#[test]
fn current_ydotool_help_selects_keycodes() {
    let help = "Usage: key [OPTION]... [KEYCODE:PRESSED]...\nEmit key events.\n\
                -d, --key-delay=N   Delay N milliseconds between key events";
    assert_eq!(YdotoolSyntax::from_key_help(help), YdotoolSyntax::Keycodes);
}
