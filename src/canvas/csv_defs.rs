// #![allow(non_snake_case)]

//! https://cloviscc.instructure.com/doc/api/file.sis_csv.html
//! https://canvas.instructure.com/doc/api/sis_imports.html

use chrono::NaiveDateTime;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ADStatus {
	Active,
	Deleted,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ASDStatus {
	Active,
	Suspended,
	Deleted,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ADCIDStatus {
	Active,
	Deleted,
	Completed,
	Inactive,
	DeletedLastComplete,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ADCPStatus {
	Active,
	Deleted,
	Completed,
	Published,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseFormat {
	OnCampus,
	Online,
	Blended,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredUserType {
	Administrative,
	Observer,
	Staff,
	Student,
	StudentOther,
	Teacher,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GradePassbackSetting {
	NightlySync,
	NotSet,
}

#[derive(Debug)]
pub enum SetDelete<T: Debug> {
	Set(T),
	Delete,
}

impl<'de, T: Debug + Deserialize<'de>> Deserialize<'de> for SetDelete<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct Visit<T>(PhantomData<T>);
		impl<'de, T: Debug + Deserialize<'de>> Visitor<'de> for Visit<T> {
			type Value = SetDelete<T>;

			fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
				formatter.write_str("\"<delete>\" or actual type (str supported so far)")
			}

			fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
			where
				E: serde::de::Error,
			{
				if v == "<delete>" {
					Ok(SetDelete::Delete)
				} else {
					Deserialize::deserialize(serde::de::value::StrDeserializer::<E>::new(v))
				}
			}
		}
		deserializer.deserialize_any(Visit(PhantomData::default()))
	}
}

impl<T: Debug + Serialize> Serialize for SetDelete<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match self {
			SetDelete::Set(t) => t.serialize(serializer),
			SetDelete::Delete => serializer.serialize_str("<delete>"),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AccountId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AuthenticationProviderId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct BlueprintCourseId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct CourseId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct IntegrationId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct LoginId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Password(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Role(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct RoleId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct SectionId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct SSHAPassword(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TermId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct UserId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct UserIntegrationId(pub String);

#[derive(Debug, Deserialize, Serialize)]
pub struct Course {
	pub course_id: CourseId,
	pub short_name: String,
	pub long_name: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub account_id: Option<AccountId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub term_id: Option<TermId>,
	pub status: ADCPStatus,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub integration_id: Option<IntegrationId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start_date: Option<SetDelete<NaiveDateTime>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end_date: Option<SetDelete<NaiveDateTime>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub course_format: Option<CourseFormat>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub blueprint_course_id: Option<BlueprintCourseId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub grade_passback_setting: Option<GradePassbackSetting>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub homeroom_course: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub friendly_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Enrollment {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub course_id: Option<CourseId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub root_account: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start_date: Option<NaiveDateTime>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end_date: Option<NaiveDateTime>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub user_id: Option<UserId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub user_integration_id: Option<UserIntegrationId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub role: Option<Role>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub role_id: Option<RoleId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub section_id: Option<SectionId>,
	pub status: ADCIDStatus,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub associated_user_id: Option<UserId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub limit_section_privileges: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub notify: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Section {
	pub section_id: SectionId,
	pub course_id: CourseId,
	pub name: String,
	pub status: ADStatus,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub integration_id: Option<IntegrationId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start_date: Option<NaiveDateTime>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end_date: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Term {
	pub term_id: TermId,
	pub name: String,
	pub status: ADStatus,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub integration_id: Option<IntegrationId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub date_override_enrollment_type: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start_date: Option<NaiveDateTime>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end_date: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
	pub user_id: UserId,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub integration_id: Option<IntegrationId>,
	pub login_id: LoginId,
	pub password: Option<Password>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub ssha_password: Option<SSHAPassword>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub authentication_provider_id: Option<AuthenticationProviderId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub first_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub last_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub full_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sortable_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub short_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub email: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pronouns: Option<SetDelete<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub declared_user_type: Option<SetDelete<DeclaredUserType>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub canvas_password_notification: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub home_account: Option<String>,
	pub status: ASDStatus,
}
