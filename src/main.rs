#![allow(non_snake_case)]

use anyhow::Context as _;
use axum::{
    Json,
    routing::get,
    Router,
};
use glob::glob;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

trait ExtendedAnyhow<T> {
    fn from_anyhow(self) -> Result<T, String>;
}

impl<T> ExtendedAnyhow<T> for anyhow::Result<T> {
    fn from_anyhow(self) -> Result<T, String> {
        match self {
            Ok(t) => Ok(t),
            Err(e) => {
                let error = format!("{e:?}");
                eprintln!("{}", &error);
                Err(error)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // TODO: Add token authorization
    // TODO: Add oracle connector (>.<)
    // TODO: Add https support with self-signed certificate
    // TODO: Automatic starting via systemd service
    // TODO: (Tokio-based?) internal cron-job functionality
    let app = Router::new()
        .route("/INT069A.json", get(route_INT069A))
        .route("/INT069B.json", get(route_INT069B))
        .route("/", get(|| async { "pong" }));

    // run it with hyper on 0.0.0.0:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

const BASE_CSV_PATH: &'static str = "/var/sftp/workday";
const BACKUP_KEEP: usize = 3;

fn get_last_matching_file_delete_rest(globsb: &str) -> anyhow::Result<PathBuf> {
    let globs = format!("{BASE_CSV_PATH}/{globsb}");
    let matches = glob(&globs)?;
    let mut paths: Vec<PathBuf> = matches.collect::<Result<_, _>>()?;
    if paths.is_empty() {
        anyhow::bail!("no matching files for {}", globsb);
    }
    paths.sort();
    if !paths.last().unwrap().is_file() {
        anyhow::bail!("last matching path is not a file: {:?}", paths.last().unwrap());
    }
    if paths.len() > BACKUP_KEEP {
        eprintln!("Too many files matching `{globsb}`, deleting old:");
        for to_del in &paths[0..(paths.len()-BACKUP_KEEP)] {
            eprintln!("\t{to_del:?}");
            std::fs::remove_file(&to_del)?;
        }
    }
    println!("Found matching paths: {paths:?}");
    Ok(paths.pop().unwrap())
}

#[derive(Debug, Deserialize, Serialize)]
struct INT069ARow {
    Employee_ID: String,
    User_Name: String,
    dateOfBirth: String,
    #[serde(rename(deserialize = "Legal_Name_-_First_Name"))] Legal_Name_First_Name: String,
    #[serde(rename(deserialize = "Legal_Name_-_Middle_Name"))] Legal_Name_Middle_Name: String,
    #[serde(rename(deserialize = "Legal_Name_-_Last_Name"))] Legal_Name_Last_Name: String,
    #[serde(rename(deserialize = "Preferred_Name_-_First_Name"))] Preferred_Name_First_Name: String,
    #[serde(rename(deserialize = "Preferred_Name_-_Middle_Name"))] Preferred_Name_Middle_Name: String,
    #[serde(rename(deserialize = "Preferred_Name_-_Last_Name"))] Preferred_Name_Last_Name: String,
    #[serde(rename(deserialize = "Email_-_Work"))] Email_Work: String,
    #[serde(rename(deserialize = "Email_-_Home"))] Email_Home: String,
    primaryWorkPhone: String,
    #[serde(rename(deserialize = "Phone_-_Primary_Home"))] Phone_Primary_Home: String,
    Primary_Mobile_Phone: String,
    #[serde(rename(deserialize = "Home_Address_-_Formatted_Line_1"))] Home_Address_Formatted_Line_1: String,
    #[serde(rename(deserialize = "Home_Address_-_Formatted_Line_2"))] Home_Address_Formatted_Line_2: String,
    #[serde(rename(deserialize = "Home_Address_-_Formatted_Line_3"))] Home_Address_Formatted_Line_3: String,
    Home_Address_city: String,
    Home_Address_State: String,
    Home_Address_Postal_Code: String,
    Legacy_Banner_ID: String,
}

async fn route_INT069A() -> Result<Json<Vec<INT069ARow>>, String> {
    println!("INT069A request");
    let file_path = get_last_matching_file_delete_rest("INT069A*.csv").from_anyhow()?;
    dbg!(&file_path);
    let mut rdr = csv::Reader::from_path(file_path).context("loading csv file").from_anyhow()?;
    let data: Vec<INT069ARow> = rdr.deserialize().collect::<Result<_, _>>().context("parsing csv file").from_anyhow()?;
    Ok(Json(data))
}

#[derive(Debug, Deserialize, Serialize)]
struct INT069BRow {
    Position_ID: String,
    #[serde(rename(deserialize = "External Position ID"))] External_Position_ID: String,
    Employee_ID: String,
    Time_Type: String,
    Employee_Type: String,
    Position_Title: String,
    Job_Profile_ID: String,
    Job_Profile_Name: String,
    Job_Family_Group: String,
    Position_Start: String,
    Position_End: String,
    Division: String,
    Department: String,
    Position_Location_Address_Line_1: String,
    Position_Location_Address_Line_2: String,
    Position_Location_Address_Line_3: String,
    Position_Location_Address_City: String,
    Position_Location_Address_State: String,
    Position_Location_Address_Postal_Code: String,
}

async fn route_INT069B() -> Result<Json<Vec<INT069BRow>>, String> {
    println!("INT069B request");
    let file_path = get_last_matching_file_delete_rest("INT069B*.csv").from_anyhow()?;
    dbg!(&file_path);
    let mut rdr = csv::Reader::from_path(file_path).context("loading csv file").from_anyhow()?;
    let data: Vec<INT069BRow> = rdr.deserialize().collect::<Result<_, _>>().context("parsing csv file").from_anyhow()?;
    Ok(Json(data))
}
