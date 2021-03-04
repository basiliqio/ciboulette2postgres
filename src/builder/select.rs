use super::*;
use crate::graph_walker::relationships::Ciboulette2PostgresRelationships;

const EMPTY_LIST: [Cow<'static, str>; 0] = [];

impl<'a> Ciboulette2PostgresBuilder<'a> {
    pub(crate) fn gen_select_cte_final(
        &mut self,
        table: &Ciboulette2PostgresTableSettings<'a>,
        type_: &'a CibouletteResourceType<'a>,
        query: &'a CibouletteQueryParameters<'a>,
        include: bool,
    ) -> Result<(), Ciboulette2SqlError> {
        // SELECT
        self.buf.write_all(b"SELECT ")?;
        // SELECT "schema"."mytable"."id"
        self.insert_ident(
            &(
                table.id_name().clone(),
                Some(Ciboulette2PostgresSafeIdent::try_from("id")?),
                Some(Ciboulette2PostgresSafeIdent::try_from("TEXT")?),
            ),
            table,
        )?;
        // SELECT "schema"."mytable"."id",
        self.buf.write_all(b", ")?;
        // SELECT "schema"."mytable"."id", $0
        self.insert_params(
            Ciboulette2SqlValue::Text(Some(Cow::Borrowed(type_.name().as_ref()))), // TODO do better
            table,
        )?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type",
        self.buf.write_all(b"::TEXT AS \"type\", ")?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..)
        self.gen_json_builder(table, type_, query, include)?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM
        self.buf.write_all(b" AS \"data\" FROM ")?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."other_table"
        self.write_table_info(table)?;
        Ok(())
    }

    pub(crate) fn gen_select_cte_single_rel(
        &mut self,
        table: &Ciboulette2PostgresTableSettings<'a>,
        type_: &'a CibouletteResourceType<'a>,
        query: &'a CibouletteQueryParameters<'a>,
        main_table: &Ciboulette2PostgresTableSettings<'a>,
        field_id: &Ciboulette2PostgresSafeIdent<'a>,
    ) -> Result<(), Ciboulette2SqlError> {
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable"
        self.gen_select_cte_final(&table, &type_, &query, query.include().contains(&type_))?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable" WHERE
        self.buf.write_all(b" WHERE ")?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable" WHERE "schema"."mytable"."id"
        self.insert_ident(&(table.id_name().clone(), None, None), &table)?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable" WHERE "schema"."mytable"."id" IN (SELECT
        self.buf.write_all(b" IN (SELECT ")?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable" WHERE "schema"."mytable"."id" IN (SELECT "schema"."othertable"."id"
        self.insert_ident(&(field_id.clone(), None, None), &main_table)?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable" WHERE "schema"."mytable"."id" IN (SELECT "schema"."othertable"."id" FROM
        self.buf.write_all(b" FROM ")?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable" WHERE "schema"."mytable"."id" IN (SELECT "schema"."othertable"."id" FROM "schema"."othertable"
        self.write_table_info(&main_table)?;
        // SELECT "schema"."mytable"."id", $0::TEXT AS "type", JSON_BUILD_OBJECT(..) AS "data" FROM "schema"."mytable" WHERE "schema"."mytable"."id" IN (SELECT "schema"."othertable"."id" FROM "schema"."othertable")
        self.buf.write_all(b")")?;
        Ok(())
    }

    pub(crate) fn gen_json_builder_routine<'b, I>(
        &mut self,
        table: &Ciboulette2PostgresTableSettings<'_>,
        obj: &'a MessyJsonObject<'a>,
        obj_name: &'b str,
        mut fields: std::iter::Peekable<I>,
    ) -> Result<(), Ciboulette2SqlError>
    where
        I: std::iter::Iterator<Item = &'a str>,
    {
        // If there is nothing, return an empty JSON object
        if fields.peek().is_none() {
            self.buf.write_all(b"NULL::json ")?;
            return Ok(());
        }
        self.buf.write_all(b"JSON_BUILD_OBJECT(")?;
        while let Some(el) = fields.next() {
            match obj.properties().get(el).ok_or_else(|| {
                CibouletteError::UnknownField(obj_name.to_string(), el.to_string())
            })? {
                MessyJson::Obj(obj) => {
                    self.gen_json_builder_routine(
                        table,
                        obj,
                        obj_name,
                        EMPTY_LIST.iter().map(Cow::as_ref).peekable(), // TODO Find a cleaner way to do that
                    )?;
                }
                _ => {
                    self.insert_params(Ciboulette2SqlValue::Text(Some(Cow::Borrowed(el))), &table)?;
                    self.buf.write_all(b", ")?;
                    self.insert_ident(
                        &(Ciboulette2PostgresSafeIdent::try_from(el)?, None, None),
                        &table,
                    )?;
                }
            }
            if fields.peek().is_some() {
                self.buf.write_all(b", ")?;
            }
        }
        self.buf.write_all(b") ")?;
        Ok(())
    }

    pub(crate) fn gen_json_builder(
        &mut self,
        table: &Ciboulette2PostgresTableSettings<'_>,
        type_: &'a CibouletteResourceType<'a>,
        query: &'a CibouletteQueryParameters<'a>,
        include: bool,
    ) -> Result<(), Ciboulette2SqlError> {
        match (query.sparse().get(type_), include) {
            (Some(fields), true) => {
                // If there is no sparse field, nothing will be returned
                self.gen_json_builder_routine(
                    table,
                    type_.schema(),
                    type_.name(),
                    fields.iter().map(Cow::as_ref).peekable(),
                )?;
            }
            (None, true) => {
                // If the sparse parameter is omitted, everything is returned
                self.gen_json_builder_routine(
                    table,
                    type_.schema(),
                    type_.name(),
                    type_
                        .schema()
                        .properties()
                        .keys()
                        .map(|x| x.as_str())
                        .peekable(),
                )?;
            }
            (_, false) => {
                // If the type is not include, return NULL::json
                self.gen_json_builder_routine(
                    table,
                    type_.schema(),
                    type_.name(),
                    vec![].into_iter().peekable(),
                )?;
            }
        };
        Ok(())
    }

    pub(crate) fn gen_union_select_all<'b, I>(
        &mut self,
        tables: I,
    ) -> Result<(), Ciboulette2SqlError>
    where
        I: IntoIterator<Item = &'b Ciboulette2PostgresTableSettings<'b>>,
    {
        let mut iter = tables.into_iter().peekable();
        while let Some(table) = iter.next() {
            // SELECT * FROM
            self.buf.write_all(b"SELECT * FROM ")?;
            // SELECT * FROM "schema"."mytable"
            self.write_table_info(table)?;
            if iter.peek().is_some() {
                // If there's more :
                // SELECT * FROM "schema"."mytable" UNION ALL ...
                self.buf.write_all(b" UNION ALL ")?;
            }
        }
        Ok(())
    }

    pub(crate) fn gen_select_rel_routine(
        &mut self,
        ciboulette_table_store: &'a Ciboulette2PostgresTableStore<'a>,
        query: &'a CibouletteQueryParameters<'a>,
        table_list: &mut Vec<Ciboulette2PostgresTableSettings<'a>>,
        main_cte_data: &Ciboulette2PostgresTableSettings<'a>,
        rels: Vec<Ciboulette2PostgresRelationships<'a>>,
    ) -> Result<(), Ciboulette2SqlError> {
        let rel_iter = rels.into_iter().peekable();
        for Ciboulette2PostgresRelationships {
            type_: rel_type,
            bucket,
            values: _rel_ids,
        } in rel_iter
        {
            self.buf.write_all(b", ")?;
            let rel_table = ciboulette_table_store.get(rel_type.name().as_str())?;
            let rel_rel_table = ciboulette_table_store.get(bucket.resource().name().as_str())?;
            let rel_cte_rel_data =
                rel_table.to_cte(Cow::Owned(format!("cte_rel_{}_rel_data", rel_table.name())))?;
            let rel_cte_data =
                rel_table.to_cte(Cow::Owned(format!("cte_rel_{}_data", rel_table.name())))?;
            // "cte_rel_myrel_rel_data"
            self.write_table_info(&rel_cte_rel_data)?;
            // "cte_rel_myrel_rel_data" AS (
            self.buf.write_all(b" AS (")?;
            // "cte_rel_myrel_rel_data" AS (select_stmt
            self.gen_select_cte_final(
                &rel_rel_table,
                &bucket.resource(),
                &query,
                query.include().contains(&bucket.resource()),
            )?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE
            self.buf.write_all(b" WHERE ")?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to"
            self.insert_ident(
                &(
                    Ciboulette2PostgresSafeIdent::try_from(bucket.to().as_str())?,
                    None,
                    None,
                ),
                &rel_rel_table,
            )?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to" =
            self.buf.write_all(b" = ")?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to" = "cte_main_data"."myid"
            self.insert_ident(
                &(main_cte_data.id_name().clone(), None, None),
                &main_cte_data,
            )?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to" = "cte_main_data"."myid"),
            self.buf.write_all(b"), ")?;
            self.write_table_info(&rel_cte_data)?;
            self.buf.write_all(b" AS (")?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to" = "cte_main_data"."myid"), "cte_rel_myrel_data" AS (select_stmt)
            self.gen_select_cte_final(
                &rel_table,
                &rel_type,
                &query,
                query.include().contains(&rel_type),
            )?;
            self.buf.write_all(b" WHERE ")?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to" = "cte_main_data"."myid"), "cte_rel_myrel_data" AS (select_stmt) WHERE "schema"."rel_table"."id" IN (SELECT \"id\" FROM
            self.insert_ident(&(rel_table.id_name().clone(), None, None), &rel_table)?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to" = "cte_main_data"."myid"), "cte_rel_myrel_data" AS (select_stmt) WHERE "schema"."rel_table"."id" IN (SELECT \"id\" FROM
            self.buf.write_all(b" IN (SELECT ")?;
            self.insert_ident(
                &(
                    Ciboulette2PostgresSafeIdent::try_from(bucket.from().as_str())?,
                    None,
                    None,
                ),
                &rel_cte_rel_data,
            )?;
            self.buf.write_all(b" FROM ")?;
            self.write_table_info(&rel_cte_rel_data)?;
            // "cte_rel_myrel_rel_data" AS (select_stmt WHERE "schema"."my_rel_rel"."to" = "cte_main_data"."myid"), "cte_rel_myrel_data" AS (select_stmt) WHERE "schema"."rel_table"."id" IN (SELECT \"id\" FROM "cte_rel_myrel_id")
            self.buf.write_all(b")")?;
            table_list.push(rel_cte_data);
            table_list.push(rel_cte_rel_data);
        }
        Ok(())
    }
}
