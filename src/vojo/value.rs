use core::str;
use skiplist::OrderedSkipList;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::LinkedList;
#[derive(PartialEq, Debug)]
pub enum Value {
    /// Nil should not be stored, but it is used as a default for initialized values
    Nil,
    String(ValueString),
    List(ValueList),
    Set(ValueSet),
    SortedSet(ValueSortedSet),
}
pub enum BackgroundEvent {
    Nil,
}
impl Value {
    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }
    pub fn is_list(&self) -> bool {
        matches!(self, Value::List(_))
    }
    pub fn is_set(&self) -> bool {
        matches!(self, Value::Set(_))
    }
    pub fn is_sorted_set(&self) -> bool {
        matches!(self, Value::SortedSet(_))
    }
    pub fn to_value_string(&self) -> Result<ValueString, anyhow::Error> {
        match self {
            Value::String(val) => Ok(val.clone()),
            _ => Err(anyhow!("convert Error!")),
        }
    }
    pub fn strlen(&self) -> Result<usize, anyhow::Error> {
        match self {
            Value::Nil => Ok(0),
            Value::String(val) => Ok(val.strlen()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn append(&mut self, newvalue: Vec<u8>) -> Result<usize, anyhow::Error> {
        match self {
            Value::Nil => {
                let len = newvalue.len();
                *self = Value::String(ValueString { data: newvalue });
                Ok(len)
            }
            Value::String(val) => {
                val.data.extend_from_slice(&newvalue);
                Ok(val.data.len())
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
}
#[derive(PartialEq, Debug, Clone)]
pub struct ValueString {
    pub data: Vec<u8>,
}
impl ValueString {
    pub fn strlen(&self) -> usize {
        self.data.len()
    }
}
#[derive(PartialEq, Debug, Clone)]
pub struct ValueList {
    data: LinkedList<Vec<u8>>,
}
#[derive(PartialEq, Debug, Clone)]
pub struct ValueSet {
    data: HashSet<Vec<u8>>,
}
#[derive(PartialEq, Debug)]
pub struct ValueSortedSet {
    // FIXME: Vec<u8> is repeated in memory
    data: OrderedSkipList<Vec<u8>>,
}
