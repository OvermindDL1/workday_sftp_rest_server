#![allow(non_snake_case)]

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

// fn date_time_weird_dash<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
// where
//     D: serde::Deserializer<'de>,
// {
//     let s = <&str>::deserialize(d)?;
//     let dt =
//         chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d-%H:%M").map_err(serde::de::Error::custom)?;
//     Ok(dt)
// }

#[derive(Debug, Deserialize, Serialize)]
pub struct INT004Row {
	// Actually Employee_ID, not Student_ID
	pub STUDENT_ID: String,
	pub LAST_NAME: String,
	pub FIRST_NAME: String,
	pub ACTIVITY_DATE: NaiveDate,
	pub BALANCE_AMOUNT: f64,
	pub AUTH_END_DATE: NaiveDate,
	pub AUTH_START_DATE: NaiveDate,
	pub AWARD_TYPE: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct INT005ARow {
	pub SSN: String, // Yes really, it's a string in banner...
	pub EmployeeID: String,
	pub FirstName: String,
	pub LastName: String,
	pub EarningType: String,
	pub Amount: f64, // Yes this it what it is in banner...
	//#[serde(deserialize_with = "date_time_weird_dash")]
	pub PeriodStartDate: NaiveDate,
	//#[serde(deserialize_with = "date_time_weird_dash")]
	pub PeriodEndDate: NaiveDate,
	//#[serde(deserialize_with = "date_time_weird_dash")]
	pub PaymentDate: NaiveDate,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct INT069ARow {
	pub Employee_ID: String,
	pub User_Name: String,
	pub dateOfBirth: String,
	#[serde(rename(deserialize = "Legal_Name_-_First_Name"))]
	pub Legal_Name_First_Name: String,
	#[serde(rename(deserialize = "Legal_Name_-_Middle_Name"))]
	pub Legal_Name_Middle_Name: String,
	#[serde(rename(deserialize = "Legal_Name_-_Last_Name"))]
	pub Legal_Name_Last_Name: String,
	#[serde(rename(deserialize = "Preferred_Name_-_First_Name"))]
	pub Preferred_Name_First_Name: String,
	#[serde(rename(deserialize = "Preferred_Name_-_Middle_Name"))]
	pub Preferred_Name_Middle_Name: String,
	#[serde(rename(deserialize = "Preferred_Name_-_Last_Name"))]
	pub Preferred_Name_Last_Name: String,
	#[serde(rename(deserialize = "Email_-_Work"))]
	pub Email_Work: String,
	#[serde(rename(deserialize = "Email_-_Home"))]
	pub Email_Home: String,
	pub primaryWorkPhone: String,
	#[serde(rename(deserialize = "Phone_-_Primary_Home"))]
	pub Phone_Primary_Home: String,
	pub Primary_Mobile_Phone: String,
	#[serde(rename(deserialize = "Home_Address_-_Formatted_Line_1"))]
	pub Home_Address_Formatted_Line_1: String,
	#[serde(rename(deserialize = "Home_Address_-_Formatted_Line_2"))]
	pub Home_Address_Formatted_Line_2: String,
	#[serde(rename(deserialize = "Home_Address_-_Formatted_Line_3"))]
	pub Home_Address_Formatted_Line_3: String,
	pub Home_Address_city: String,
	pub Home_Address_State: String,
	pub Home_Address_Postal_Code: String,
	pub Legacy_Banner_ID: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct INT069BRow {
	pub Position_ID: String,
	#[serde(rename(deserialize = "External Position ID"))]
	pub External_Position_ID: String,
	pub Employee_ID: String,
	pub Time_Type: String,
	pub Employee_Type: String,
	pub Position_Title: String,
	pub Job_Profile_ID: String,
	pub Job_Profile_Name: String,
	pub Job_Family_Group: String,
	pub Position_Start: String,
	pub Position_End: String,
	pub Division: String,
	pub Department: String,
	pub Position_Location_Address_Line_1: String,
	pub Position_Location_Address_Line_2: String,
	pub Position_Location_Address_Line_3: String,
	pub Position_Location_Address_City: String,
	pub Position_Location_Address_State: String,
	pub Position_Location_Address_Postal_Code: String,
	pub Cost_Center_Code: String,
	pub Cost_Center_Description: String,
	pub Manager_Employee_ID: String,
}
