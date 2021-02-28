use super::*;

#[test]
fn simple() {
    let mut builder = Ciboulette2PostgresBuilder::new();
    let dest_table = CibouletteTableSettings::new(
        Cow::Borrowed("id"),
        Cow::Borrowed("uuid"),
        Some(Cow::Borrowed("public")),
        Cow::Borrowed("peoples"),
    );
    let store = gen_bag();
    let url = Url::parse("http://localhost/peoples").unwrap();
    let ciboulette_request = gen_req_create_people(&store, &url);
    builder
        .gen_select_cte_final(
            &dest_table,
            store.get_type("peoples").unwrap(),
            ciboulette_request.query(),
        )
        .unwrap();

    let res = builder.build().unwrap();
    insta::assert_debug_snapshot!(res);
}

#[test]
fn sparse() {
    let mut builder = Ciboulette2PostgresBuilder::new();
    let dest_table = CibouletteTableSettings::new(
        Cow::Borrowed("id"),
        Cow::Borrowed("uuid"),
        Some(Cow::Borrowed("public")),
        Cow::Borrowed("peoples"),
    );
    let store = gen_bag();
    let url = Url::parse("http://localhost/peoples?fields[peoples]=first-name").unwrap();
    let ciboulette_request = gen_req_create_people(&store, &url);
    builder
        .gen_select_cte_final(
            &dest_table,
            store.get_type("peoples").unwrap(),
            ciboulette_request.query(),
        )
        .unwrap();

    let res = builder.build().unwrap();
    insta::assert_debug_snapshot!(res);
}

#[test]
fn empty_sparse() {
    let mut builder = Ciboulette2PostgresBuilder::new();
    let dest_table = CibouletteTableSettings::new(
        Cow::Borrowed("id"),
        Cow::Borrowed("uuid"),
        Some(Cow::Borrowed("public")),
        Cow::Borrowed("peoples"),
    );
    let store = gen_bag();
    let url = Url::parse("http://localhost/peoples?fields[peoples]=").unwrap();
    let ciboulette_request = gen_req_create_people(&store, &url);
    builder
        .gen_select_cte_final(
            &dest_table,
            store.get_type("peoples").unwrap(),
            ciboulette_request.query(),
        )
        .unwrap();

    let res = builder.build().unwrap();
    insta::assert_debug_snapshot!(res);
}
