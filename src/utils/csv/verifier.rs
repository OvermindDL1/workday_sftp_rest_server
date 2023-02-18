use serde::ser::{
	SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple, SerializeTupleStruct,
	SerializeTupleVariant,
};
use serde::{Serialize, Serializer};
use std::fmt::Display;

#[derive(Debug)]
pub struct Verifier {
	verify: bool,
	fields: Vec<&'static str>,
	skipped_fields: Vec<&'static str>,
	fields_so_far: Vec<&'static str>,
	skipped_fields_so_far: Vec<&'static str>,
}

impl Verifier {
	pub fn new() -> Verifier {
		Verifier {
			verify: false,
			fields: Vec::new(),
			skipped_fields: Vec::new(),
			fields_so_far: Vec::new(),
			skipped_fields_so_far: Vec::new(),
		}
	}

	pub fn reset_to_verify(&mut self) -> anyhow::Result<()> {
		if !self.verify {
			if self.fields_so_far.is_empty() {
				anyhow::bail!("no fields to verify");
			}
			self.verify = true;
			std::mem::swap(&mut self.fields, &mut self.fields_so_far);
			self.fields_so_far.reserve(self.fields.len());
			std::mem::swap(&mut self.skipped_fields, &mut self.skipped_fields_so_far);
			return Ok(());
		}
		if self.fields != self.fields_so_far {
			anyhow::bail!("fields do not match: {:?} != {:?}", self.fields, self.fields_so_far);
		}
		if self.skipped_fields != self.skipped_fields_so_far {
			anyhow::bail!(
				"skipped fields do not match: {:?} != {:?}",
				self.skipped_fields,
				self.skipped_fields_so_far
			);
		}
		Ok(())
	}
}

#[derive(Debug)]
pub struct VerifierError(anyhow::Error);
impl Display for VerifierError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.0, f)
	}
}
impl std::error::Error for VerifierError {}
impl serde::ser::Error for VerifierError {
	fn custom<T: std::fmt::Display>(msg: T) -> Self {
		VerifierError(anyhow::anyhow!("{msg}"))
	}
}

impl SerializeSeq for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;

	fn serialize_element<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("sequences are not supported")))
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("sequences are not supported")))
	}
}

impl SerializeTuple for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;

	fn serialize_element<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("tuples are not supported")))
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("tuples are not supported")))
	}
}

impl SerializeTupleStruct for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;

	fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("tuplestructs are not supported")))
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("tuplestructs are not supported")))
	}
}

impl SerializeTupleVariant for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;

	fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("tuplevariants are not supported")))
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("tuplevariants are not supported")))
	}
}

impl SerializeMap for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;

	fn serialize_key<T: ?Sized>(&mut self, _key: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("maps are not supported")))
	}

	fn serialize_value<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("maps are not supported")))
	}

	fn serialize_entry<K: ?Sized, V: ?Sized>(&mut self, _key: &K, _value: &V) -> Result<(), Self::Error>
	where
		K: Serialize,
		V: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("maps are not supported")))
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("maps are not supported")))
	}
}

impl SerializeStructVariant for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;

	fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, _value: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("structvariants are not supported")))
	}

	fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
		Err(VerifierError(anyhow::anyhow!("structvariants are not supported")))
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("structvariants are not supported")))
	}
}

impl SerializeStruct for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;

	fn serialize_field<T: ?Sized>(&mut self, key: &'static str, _value: &T) -> Result<(), Self::Error>
	where
		T: Serialize,
	{
		self.fields_so_far.push(key);
		Ok(())
	}

	fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
		self.skipped_fields_so_far.push(key);
		Ok(())
	}

	fn end(self) -> Result<Self::Ok, Self::Error> {
		Ok(self)
	}
}

impl Serializer for Verifier {
	type Ok = Verifier;
	type Error = VerifierError;
	type SerializeSeq = Self;
	type SerializeTuple = Self;
	type SerializeTupleStruct = Self;
	type SerializeTupleVariant = Self;
	type SerializeMap = Self;
	type SerializeStruct = Self;
	type SerializeStructVariant = Self;

	fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("bool are not supported")))
	}

	fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("i8 are not supported")))
	}

	fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("i16 are not supported")))
	}

	fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("i32 are not supported")))
	}

	fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("i64 are not supported")))
	}

	fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("u8 are not supported")))
	}

	fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("u16 are not supported")))
	}

	fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("u32 are not supported")))
	}

	fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("u64 are not supported")))
	}

	fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("f32 are not supported")))
	}

	fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("f64 are not supported")))
	}

	fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("char are not supported")))
	}

	fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("str are not supported")))
	}

	fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("[u8] are not supported")))
	}

	fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("None are not supported")))
	}

	fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("Some are not supported")))
	}

	fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("unit are not supported")))
	}

	fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("unitstructs are not supported")))
	}

	fn serialize_unit_variant(
		self,
		_name: &'static str,
		_variant_index: u32,
		_variant: &'static str,
	) -> Result<Self::Ok, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("unit variants are not supported")))
	}

	fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, _value: &T) -> Result<Self::Ok, Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("newtype structs are not supported")))
	}

	fn serialize_newtype_variant<T: ?Sized>(
		self,
		_name: &'static str,
		_variant_index: u32,
		_variant: &'static str,
		_value: &T,
	) -> Result<Self::Ok, Self::Error>
	where
		T: Serialize,
	{
		Err(VerifierError(anyhow::anyhow!("newtype variants are not supported")))
	}

	fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("seq are not supported")))
	}

	fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("tuples are not supported")))
	}

	fn serialize_tuple_struct(
		self,
		_name: &'static str,
		_len: usize,
	) -> Result<Self::SerializeTupleStruct, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("tuple structs are not supported")))
	}

	fn serialize_tuple_variant(
		self,
		_name: &'static str,
		_variant_index: u32,
		_variant: &'static str,
		_len: usize,
	) -> Result<Self::SerializeTupleVariant, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("tuple variants are not supported")))
	}

	fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("maps are not supported")))
	}

	fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
		if self.fields_so_far.is_empty() {
			Ok(self)
		} else {
			Err(VerifierError(anyhow::anyhow!("embedded structs are not supported")))
		}
	}

	fn serialize_struct_variant(
		self,
		_name: &'static str,
		_variant_index: u32,
		_variant: &'static str,
		_len: usize,
	) -> Result<Self::SerializeStructVariant, Self::Error> {
		Err(VerifierError(anyhow::anyhow!("struct variants are not supported")))
	}
}
