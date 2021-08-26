use anyhow::{Result, bail};
use ballista::context::BallistaContext;
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::prelude::*;
use std::include_str;

pub fn get_query(tpch_query: u8) -> Result<&'static str> {
    match tpch_query {
        1 => Ok(include_str!("tpch_queries/q1.sql")),
        2 => Ok(include_str!("tpch_queries/q2.sql")),
        3 => Ok(include_str!("tpch_queries/q3.sql")),
        4 => Ok(include_str!("tpch_queries/q4.sql")),
        5 => Ok(include_str!("tpch_queries/q5.sql")),
        6 => Ok(include_str!("tpch_queries/q6.sql")),
        7 => Ok(include_str!("tpch_queries/q7.sql")),
        8 => Ok(include_str!("tpch_queries/q8.sql")),
        9 => Ok(include_str!("tpch_queries/q9.sql")),
        10 => Ok(include_str!("tpch_queries/q10.sql")),
        11 => Ok(include_str!("tpch_queries/q11.sql")),
        12 => Ok(include_str!("tpch_queries/q12.sql")),
        13 => Ok(include_str!("tpch_queries/q13.sql")),
        14 => Ok(include_str!("tpch_queries/q14.sql")),
        15 => Ok(include_str!("tpch_queries/q15.sql")),
        16 => Ok(include_str!("tpch_queries/q16.sql")),
        17 => Ok(include_str!("tpch_queries/q17.sql")),
        18 => Ok(include_str!("tpch_queries/q18.sql")),
        19 => Ok(include_str!("tpch_queries/q19.sql")),
        20 => Ok(include_str!("tpch_queries/q20.sql")),
        21 => Ok(include_str!("tpch_queries/q21.sql")),
        22 => Ok(include_str!("tpch_queries/q22.sql")),
        _ => bail!("unknown tpch query {}", tpch_query)
    }
}

const TABLES: &[&str] = &[
    "part", "supplier", "partsupp", "customer", "orders", "lineitem", "nation", "region",
];

fn get_schema(table: &str) -> Schema {
    match table {
        "part" => Schema::new(vec![
            Field::new("p_partkey", DataType::Int32, false),
            Field::new("p_name", DataType::Utf8, false),
            Field::new("p_mfgr", DataType::Utf8, false),
            Field::new("p_brand", DataType::Utf8, false),
            Field::new("p_type", DataType::Utf8, false),
            Field::new("p_size", DataType::Int32, false),
            Field::new("p_container", DataType::Utf8, false),
            Field::new("p_retailprice", DataType::Float64, false),
            Field::new("p_comment", DataType::Utf8, false),
        ]),

        "supplier" => Schema::new(vec![
            Field::new("s_suppkey", DataType::Int32, false),
            Field::new("s_name", DataType::Utf8, false),
            Field::new("s_address", DataType::Utf8, false),
            Field::new("s_nationkey", DataType::Int32, false),
            Field::new("s_phone", DataType::Utf8, false),
            Field::new("s_acctbal", DataType::Float64, false),
            Field::new("s_comment", DataType::Utf8, false),
        ]),

        "partsupp" => Schema::new(vec![
            Field::new("ps_partkey", DataType::Int32, false),
            Field::new("ps_suppkey", DataType::Int32, false),
            Field::new("ps_availqty", DataType::Int32, false),
            Field::new("ps_supplycost", DataType::Float64, false),
            Field::new("ps_comment", DataType::Utf8, false),
        ]),

        "customer" => Schema::new(vec![
            Field::new("c_custkey", DataType::Int32, false),
            Field::new("c_name", DataType::Utf8, false),
            Field::new("c_address", DataType::Utf8, false),
            Field::new("c_nationkey", DataType::Int32, false),
            Field::new("c_phone", DataType::Utf8, false),
            Field::new("c_acctbal", DataType::Float64, false),
            Field::new("c_mktsegment", DataType::Utf8, false),
            Field::new("c_comment", DataType::Utf8, false),
        ]),

        "orders" => Schema::new(vec![
            Field::new("o_orderkey", DataType::Int32, false),
            Field::new("o_custkey", DataType::Int32, false),
            Field::new("o_orderstatus", DataType::Utf8, false),
            Field::new("o_totalprice", DataType::Float64, false),
            Field::new("o_orderdate", DataType::Date32, false),
            Field::new("o_orderpriority", DataType::Utf8, false),
            Field::new("o_clerk", DataType::Utf8, false),
            Field::new("o_shippriority", DataType::Int32, false),
            Field::new("o_comment", DataType::Utf8, false),
        ]),

        "lineitem" => Schema::new(vec![
            Field::new("l_orderkey", DataType::Int32, false),
            Field::new("l_partkey", DataType::Int32, false),
            Field::new("l_suppkey", DataType::Int32, false),
            Field::new("l_linenumber", DataType::Int32, false),
            Field::new("l_quantity", DataType::Float64, false),
            Field::new("l_extendedprice", DataType::Float64, false),
            Field::new("l_discount", DataType::Float64, false),
            Field::new("l_tax", DataType::Float64, false),
            Field::new("l_returnflag", DataType::Utf8, false),
            Field::new("l_linestatus", DataType::Utf8, false),
            Field::new("l_shipdate", DataType::Date32, false),
            Field::new("l_commitdate", DataType::Date32, false),
            Field::new("l_receiptdate", DataType::Date32, false),
            Field::new("l_shipinstruct", DataType::Utf8, false),
            Field::new("l_shipmode", DataType::Utf8, false),
            Field::new("l_comment", DataType::Utf8, false),
        ]),

        "nation" => Schema::new(vec![
            Field::new("n_nationkey", DataType::Int32, false),
            Field::new("n_name", DataType::Utf8, false),
            Field::new("n_regionkey", DataType::Int32, false),
            Field::new("n_comment", DataType::Utf8, false),
        ]),

        "region" => Schema::new(vec![
            Field::new("r_regionkey", DataType::Int32, false),
            Field::new("r_name", DataType::Utf8, false),
            Field::new("r_comment", DataType::Utf8, false),
        ]),

        _ => unimplemented!(),
    }
}

pub fn register_simple_tpch_tables(ctx: &mut BallistaContext) -> Result<()> {
    for table in TABLES {
        let path = format!("/mnt/data/{}.tbl", table);
        let schema = get_schema(table);
        let options = CsvReadOptions::new()
            .schema(&schema)
            .delimiter(b'|')
            .has_header(false)
            .file_extension(".tbl");
        ctx.register_csv(table, &path, options)?;
    }
    Ok(())
}

pub fn register_memsql_tpch_tables(ctx: &mut BallistaContext) -> Result<()> {
    for table in TABLES {
        let path = format!("/mnt/data/{}/", table);
        let schema = get_schema(table);
        let options = CsvReadOptions::new()
            .schema(&schema)
            .delimiter(b'|')
            .has_header(false)
            .file_extension(".tbl");
        ctx.register_csv(table, &path, options)?;
    }
    Ok(())
}
