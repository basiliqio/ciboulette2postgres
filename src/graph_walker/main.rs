use super::*;
use std::convert::TryFrom;

/// Informations about the main resource type, extracted from the request
#[derive(Clone, Debug, Default, Getters)]
#[getset(get = "pub")]
pub(crate) struct Ciboulette2PostgresMainResourceInformations<'a> {
    pub insert_values: Vec<(Ciboulette2PostgresStr<'a>, Ciboulette2SqlValue<'a>)>,
    pub single_relationships: Vec<Ciboulette2PostgresStr<'a>>,
}

/// Check the informations of a one-to-many relationship
fn check_one_to_many_relationships<'a>(
    relationships: &'a BTreeMap<ArcStr, CibouletteRelationshipObject<'a>>,
    from_type_: Arc<CibouletteResourceType<'a>>,
    to_type_: Arc<CibouletteResourceType<'a>>,
    to_type_alias: &Ciboulette2PostgresStr<'a>,
    opt: &'a CibouletteRelationshipOneToManyOption,
) -> Result<Option<(&'a str, Ciboulette2SqlValue<'a>)>, Ciboulette2SqlError> {
    match relationships
        .get(&**to_type_alias)
        .map(|x| x.data())
        .unwrap_or(&CibouletteOptionalData::Null(false))
    {
        CibouletteOptionalData::Object(CibouletteResourceIdentifierSelector::One(rel_id)) => {
            Ok(Some((
                opt.many_table_key().as_str(),
                Ciboulette2SqlValue::from(rel_id.id()),
            )))
        }
        CibouletteOptionalData::Object(CibouletteResourceIdentifierSelector::Many(_)) => {
            return Err(Ciboulette2SqlError::RequiredSingleRelationship(
                to_type_.name().to_string(),
            ));
        }
        CibouletteOptionalData::Null(x) if *x => {
            if !opt.optional() {
                return Err(Ciboulette2SqlError::MissingRelationship(
                    from_type_.name().to_string(),
                    to_type_.name().to_string(),
                ));
            }
            match opt.one_table().id_type() {
                CibouletteIdType::Number => Ok(Some((
                    opt.many_table_key().as_str(),
                    Ciboulette2SqlValue::Numeric(None),
                ))),
                CibouletteIdType::Uuid => Ok(Some((
                    opt.many_table_key().as_str(),
                    Ciboulette2SqlValue::Uuid(None),
                ))),
                CibouletteIdType::Text => Ok(Some((
                    opt.many_table_key().as_str(),
                    Ciboulette2SqlValue::Text(None),
                ))),
            }
        }
        CibouletteOptionalData::Null(_) => {
            if !opt.optional() {
                return Err(Ciboulette2SqlError::MissingRelationship(
                    from_type_.name().to_string(),
                    to_type_.name().to_string(),
                ));
            }
            Ok(None)
        }
    }
}

/// Extract attributes from the request and push them to an arguments vector
/// compatible with SQLx for later execution
pub fn fill_attributes<'a>(
    args: &mut Vec<(Ciboulette2PostgresStr<'a>, Ciboulette2SqlValue<'a>)>,
    obj: &'a Option<MessyJsonObjectValue<'a>>,
) -> Result<(), Ciboulette2SqlError> {
    if let Some(obj) = obj {
        for (k, v) in obj.iter() {
            if matches!(v, MessyJsonValue::Null(MessyJsonNullType::Absent, _)) {
                continue;
            }
            // Iterate over every attribute
            args.push((
                Ciboulette2PostgresStr::from(k.clone()),
                Ciboulette2SqlValue::try_from(v)?,
            ));
        }
    }
    Ok(())
}

/// Extract attribute and single relationships from the query, allowing to build the
/// request for the main resource
pub(crate) fn extract_fields_and_values<'a>(
    store: &'a CibouletteStore<'a>,
    main_type: Arc<CibouletteResourceType<'a>>,
    attributes: &'a Option<MessyJsonObjectValue<'a>>,
    relationships: &'a BTreeMap<ArcStr, CibouletteRelationshipObject<'a>>,
    fails_on_many: bool,
) -> Result<Ciboulette2PostgresMainResourceInformations<'a>, Ciboulette2SqlError> {
    let mut res_val: Vec<(Ciboulette2PostgresStr<'a>, Ciboulette2SqlValue<'a>)> =
        Vec::with_capacity(128);
    let mut res_rel: Vec<Ciboulette2PostgresStr<'a>> = Vec::with_capacity(128);
    let main_type_index = store
        .get_type_index(main_type.name())
        .ok_or_else(|| CibouletteError::UnknownType(main_type.name().to_string()))?;

    fill_attributes(&mut res_val, &attributes)?;
    let mut walker = store
        .graph()
        .neighbors_directed(*main_type_index, petgraph::Direction::Outgoing)
        .detach(); // Create a graph walker
    while let Some((edge_index, node_index)) = walker.next(&store.graph()) {
        // For each connect edge outgoing from the original node
        let node_weight = store.graph().node_weight(node_index).unwrap(); // Get the node weight
        let alias =
            Ciboulette2PostgresStr::from(main_type.get_alias(node_weight.name().as_str())?.clone()); // Get the alias translation of that resource

        match store.graph().edge_weight(edge_index).unwrap() // FIXME
		{
			CibouletteRelationshipOption::ManyToOne(opt) |CibouletteRelationshipOption::OneToMany(opt) if opt.part_of_many_to_many().is_none() && opt.many_table().as_ref() == main_type.as_ref() => {
				if let Some(v) =
                check_one_to_many_relationships(&relationships, main_type.clone(), node_weight.clone(), &alias, opt)?
            {
                res_val.push((Ciboulette2PostgresStr::from(v.0), v.1)); // Insert the relationship values
            }
            res_rel.push(alias);
			},
			_ => {
				if fails_on_many  && relationships.contains_key(&*alias){
					return Err(Ciboulette2SqlError::UpdatingManyRelationships);
				}
			}
		}
    }
    Ok(Ciboulette2PostgresMainResourceInformations {
        insert_values: res_val,
        single_relationships: res_rel,
    })
}

/// Get a list of a resource's single relationships (one-to-one)
pub(crate) fn get_resource_single_rel<'a>(
    store: &'a CibouletteStore<'a>,
    main_type: Arc<CibouletteResourceType<'a>>,
) -> Result<Vec<Ciboulette2PostgresStr<'a>>, Ciboulette2SqlError> {
    let mut res: Vec<Ciboulette2PostgresStr<'a>> = Vec::with_capacity(128);
    let main_type_index = store
        .get_type_index(main_type.name())
        .ok_or_else(|| CibouletteError::UnknownType(main_type.name().to_string()))?;

    let mut walker = store
        .graph()
        .neighbors_directed(*main_type_index, petgraph::Direction::Outgoing)
        .detach(); // Create a graph walker
    while let Some((edge_index, node_index)) = walker.next(&store.graph()) {
        // For each connect edge outgoing from the original node
        let node_weight = store.graph().node_weight(node_index).unwrap(); // Get the node weight
        if let CibouletteRelationshipOption::ManyToOne(_) =
            store.graph().edge_weight(edge_index).unwrap()
        {
            let alias = main_type.get_alias(node_weight.name().as_str())?; // Get the alias translation of that resource
            res.push(Ciboulette2PostgresStr::from(alias.clone()));
        }
    }
    Ok(res)
}
