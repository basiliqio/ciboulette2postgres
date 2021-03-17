use super::*;

impl<'a> Ciboulette2PostgresBuilder<'a> {
    pub fn gen_select_normal(
        ciboulette_store: &'a CibouletteStore<'a>,
        ciboulette_table_store: &'a Ciboulette2PostgresTableStore<'a>,
        request: &'a CibouletteReadRequest<'a>,
    ) -> Result<Self, Ciboulette2SqlError> {
        let mut se = Self::default();
        let state = Ciboulette2PostgresBuilderState::new(
            ciboulette_store,
            ciboulette_table_store,
            request.path(),
            request.query(),
        )?;
        let main_cte_data = state.main_table().to_cte(Cow::Owned(format!(
            "cte_{}_data",
            state.main_table().name()
        )))?;
        let rels = Self::get_relationships(&ciboulette_store, &state.main_type())?;

        // WITH
        se.buf.write_all(b"WITH \n")?;
        // WITH "cte_main_insert"
        se.write_table_info(&main_cte_data)?;
        // WITH "cte_main_insert" AS (
        se.buf.write_all(b" AS (")?;
        se.gen_select_cte_final(
            &state.main_table(),
            &state.main_type(),
            &request.query(),
            rels.single_rels_additional_fields().iter(),
            true,
        )?;
        match request.path() {
            CiboulettePath::TypeId(_, id)
            | CiboulettePath::TypeIdRelated(_, id, _)
            | CiboulettePath::TypeIdRelationship(_, id, _) => {
                se.buf.write_all(b" WHERE ")?;
                se.insert_ident(
                    &Ciboulette2PostgresTableField::new_ref(
                        state.main_table().id().get_ident(),
                        None,
                        None,
                    ),
                    &state.main_table(),
                )?;
                se.buf.write_all(b" = ")?;
                se.insert_params(Ciboulette2SqlValue::from(id), &state.main_table())?;
            }
            _ => (),
        }
        se.buf.write_all(b")")?;

        se.gen_select_single_rel_routine(
            &ciboulette_store,
            &ciboulette_table_store,
            request.query(),
            &state.main_type(),
            &main_cte_data,
            &rels,
        )?;
        se.gen_select_multi_rel_routine(
            &ciboulette_table_store,
            request.query(),
            &main_cte_data,
            &rels.multi_rels(),
        )?;
        se.gen_cte_for_sort(
            &ciboulette_store,
            &ciboulette_table_store,
            &request.query(),
            &state.main_type(),
            &state.main_table(),
            &main_cte_data,
        )?;
        se.add_working_table(&state.main_table(), main_cte_data);
        // Aggregate every table using UNION ALL
        se.gen_union_select_all(
            &ciboulette_store,
            &ciboulette_table_store,
            &request.query(),
            &state.main_table(),
        )?;
        Ok(se)
    }
}
