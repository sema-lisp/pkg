use serde_json::Value;

#[test]
fn textmate_assets_define_the_canonical_regex_scope() {
    let prototype: Value = serde_json::from_str(include_str!("../prototypes/sema.tmLanguage.json"))
        .expect("prototype grammar must be valid JSON");
    let static_grammar: Value =
        serde_json::from_str(include_str!("../static/sema.tmLanguage.json"))
            .expect("static grammar must be valid JSON");

    assert_eq!(prototype, static_grammar);
    assert_eq!(
        prototype["repository"]["regex"]["name"],
        "string.regexp.sema"
    );
    assert_eq!(prototype["repository"]["regex"]["begin"], "#\"");
    assert_eq!(prototype["repository"]["regex"]["end"], "\"");
}

#[test]
fn syntect_asset_defines_the_regex_context_before_strings() {
    let syntax = include_str!("../syntaxes/Sema.sublime-syntax");
    let regex = syntax.find("    - include: regex").expect("regex include");
    let string = syntax
        .find("    - include: string")
        .expect("string include");

    assert!(regex < string);
    assert!(syntax.contains("meta_scope: string.regexp.sema"));
}
