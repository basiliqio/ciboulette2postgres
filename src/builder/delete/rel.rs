use super::*;

impl<'request> Ciboulette2PostgresBuilder<'request> {
    pub(super) fn gen_delete_rel_one_to_many(
        &mut self,
        table_store: &Ciboulette2PostgresTableStore,
        query: &'request CibouletteDeleteRequest<'request>,
        rel_opt: &CibouletteRelationshipOneToManyOption,
    ) -> Result<(), Ciboulette2SqlError> {
        let many_table = table_store.get(rel_opt.many_table().name().as_str())?;

        self.buf.write_all(b"UPDATE ")?;
        self.write_table_info(many_table)?;
        self.buf.write_all(b" SET ")?;
        self.insert_ident_name(
            &Ciboulette2PostgresTableField::new(
                Ciboulette2PostgresSafeIdent::try_from(rel_opt.many_table_key())?,
                None,
                None,
            ),
            &many_table,
        )?;
        self.buf.write_all(b" = NULL WHERE ")?;
        self.insert_ident(
            &Ciboulette2PostgresTableField::new(many_table.id().get_ident().clone(), None, None),
            &many_table,
        )?;
        self.buf.write_all(b" = ")?;
        self.insert_params(Ciboulette2SqlValue::from(query.resource_id()), &many_table)?;
        Ok(())
    }
}
