use crate::parser::response::Response;

use rkyv::{Archive, Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::vec;

#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub enum Value {
    /// Nil should not be stored, but it is used as a default for initialized values
    Nil,
    String(ValueString),
    List(ValueList),
    Set(ValueSet),
    Hash(ValueHash),
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
    pub fn is_hash(&self) -> bool {
        matches!(self, Value::Hash(_))
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
    pub fn to_value_hash(&self) -> Result<ValueHash, anyhow::Error> {
        match self {
            Value::Hash(val) => Ok(val.clone()),
            _ => Err(anyhow!("convert Error!")),
        }
    }
    pub fn to_value_set(&self) -> Result<ValueSet, anyhow::Error> {
        match self {
            Value::Set(val) => Ok(val.clone()),
            _ => Err(anyhow!("convert Error!")),
        }
    }
    pub fn to_value_list_mut(&mut self) -> Result<&mut ValueList, anyhow::Error> {
        match self {
            Value::List(val) => Ok(val),
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
    pub fn lpush(&mut self, newvalue: Vec<u8>) -> Result<usize, anyhow::Error> {
        match self {
            Value::List(val) => {
                val.data.push_front(newvalue);
                Ok(val.data.len())
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn rpush(&mut self, newvalue: Vec<u8>) -> Result<usize, anyhow::Error> {
        match self {
            Value::List(val) => {
                val.data.push_back(newvalue);
                Ok(val.data.len())
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn sadd(&mut self, newvalue: Vec<u8>) -> Result<bool, anyhow::Error> {
        match self {
            Value::Set(val) => {
                if val.data.contains(&newvalue) {
                    Ok(false)
                } else {
                    val.data.insert(newvalue);
                    Ok(true)
                }
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn hset(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<bool, anyhow::Error> {
        match self {
            Value::Hash(val) => {
                if let std::collections::hash_map::Entry::Vacant(e) = val.data.entry(key) {
                    e.insert(value);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn zadd(&mut self, member: Vec<u8>, score: f64) -> Result<bool, anyhow::Error> {
        match self {
            Value::SortedSet(val) => {
                val.data.insert(SortedSetData { member, score });
                Ok(true)
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn lpop(&mut self, count_option: Option<i64>) -> Result<Response, anyhow::Error> {
        match self {
            Value::List(val) => {
                if let Some(count) = count_option {
                    let mut responses = vec![];
                    for _i in 0..count {
                        let data = val.data.pop_front().ok_or(anyhow!("no data"))?;
                        responses.push(Response::Data(data));
                    }
                    Ok(Response::Array(responses))
                } else {
                    let data = val.data.pop_front().ok_or(anyhow!("no data"))?;
                    Ok(Response::Data(data))
                }
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn rpop(&mut self, count_option: Option<i64>) -> Result<Response, anyhow::Error> {
        match self {
            Value::List(val) => {
                if let Some(count) = count_option {
                    let mut responses = vec![];
                    for _i in 0..count {
                        let data = val.data.pop_back().ok_or(anyhow!("no data"))?;
                        responses.push(Response::Data(data));
                    }
                    Ok(Response::Array(responses))
                } else {
                    let data = val.data.pop_back().ok_or(anyhow!("no data"))?;
                    Ok(Response::Data(data))
                }
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn lrange(&self, mut start: i64, mut stop: i64) -> Result<Response, anyhow::Error> {
        match self {
            Value::List(val) => {
                let mut responses = vec![];
                if start < 0 {
                    start = 0;
                }
                if stop >= (val.data.len() as i64) {
                    stop = (val.data.len() as i64) - 1;
                }

                for (index, item) in val.data.iter().enumerate() {
                    if index as i64 >= start && index as i64 <= stop {
                        responses.push(Response::Data(item.clone()));
                    }
                }

                Ok(Response::Array(responses))
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn llen(&self) -> Result<usize, anyhow::Error> {
        match self {
            Value::List(val) => Ok(val.data.len()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    // Set operations
    pub fn srem(&mut self, value: Vec<u8>) -> Result<bool, anyhow::Error> {
        match self {
            Value::Set(val) => Ok(val.data.remove(&value)),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn sismember(&self, value: &[u8]) -> Result<bool, anyhow::Error> {
        match self {
            Value::Set(val) => Ok(val.data.contains(value)),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn scard(&self) -> Result<usize, anyhow::Error> {
        match self {
            Value::Set(val) => Ok(val.data.len()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn smembers(&self) -> Result<Vec<Vec<u8>>, anyhow::Error> {
        match self {
            Value::Set(val) => Ok(val.data.iter().cloned().collect()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    // Hash operations
    pub fn hget(&self, field: &[u8]) -> Result<Option<Vec<u8>>, anyhow::Error> {
        match self {
            Value::Hash(val) => Ok(val.data.get(field).cloned()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn hdel(&mut self, field: &[u8]) -> Result<bool, anyhow::Error> {
        match self {
            Value::Hash(val) => Ok(val.data.remove(field).is_some()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn hexists(&self, field: &[u8]) -> Result<bool, anyhow::Error> {
        match self {
            Value::Hash(val) => Ok(val.data.contains_key(field)),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn hlen(&self) -> Result<usize, anyhow::Error> {
        match self {
            Value::Hash(val) => Ok(val.data.len()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn hgetall(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, anyhow::Error> {
        match self {
            Value::Hash(val) => Ok(val.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    // Sorted Set operations
    pub fn zrange(&self, start: i64, stop: i64) -> Result<Vec<Vec<u8>>, anyhow::Error> {
        match self {
            Value::SortedSet(val) => {
                let data: Vec<_> = val.data.iter().collect();
                let len = data.len() as i64;
                let start = if start < 0 {
                    (len + start).max(0)
                } else {
                    start
                };
                let stop = if stop < 0 {
                    (len + stop).max(-1)
                } else {
                    stop.min(len - 1)
                };
                if start > stop {
                    return Ok(vec![]);
                }
                let result: Vec<Vec<u8>> = data[start as usize..=(stop as usize)]
                    .iter()
                    .map(|s| s.member.clone())
                    .collect();
                Ok(result)
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn zrem(&mut self, member: &[u8]) -> Result<bool, anyhow::Error> {
        match self {
            Value::SortedSet(val) => {
                let original_len = val.data.len();
                val.data.retain(|s| s.member != member);
                Ok(val.data.len() != original_len)
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn zcard(&self) -> Result<usize, anyhow::Error> {
        match self {
            Value::SortedSet(val) => Ok(val.data.len()),
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
    pub fn zscore(&self, member: &[u8]) -> Result<Option<f64>, anyhow::Error> {
        match self {
            Value::SortedSet(val) => {
                for item in &val.data {
                    if item.member == member {
                        return Ok(Some(item.score));
                    }
                }
                Ok(None)
            }
            _ => Err(anyhow!("WrongTypeError")),
        }
    }
}
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ValueString {
    pub data: Vec<u8>,
}
impl ValueString {
    pub fn strlen(&self) -> usize {
        self.data.len()
    }
}
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ValueList {
    pub data: VecDeque<Vec<u8>>,
}

#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ValueSet {
    pub data: HashSet<Vec<u8>>,
}
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ValueHash {
    pub data: HashMap<Vec<u8>, Vec<u8>>,
}
#[derive(Archive, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ValueSortedSet {
    pub data: BTreeSet<SortedSetData>,
}

#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[rkyv(derive(Debug))]
pub struct SortedSetData {
    pub member: Vec<u8>,
    pub score: f64,
}
impl PartialEq for SortedSetData {
    fn eq(&self, other: &Self) -> bool {
        self.member == other.member && self.score == other.score
    }
}
impl Eq for SortedSetData {}
impl PartialOrd for SortedSetData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for SortedSetData {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.score.partial_cmp(&other.score) {
            Some(ordering) => {
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            None => return Ordering::Less,
        }
        self.member.cmp(&other.member)
    }
}

// rkyv generates `ArchivedSortedSetData` but cannot auto-derive `Ord`
// because `ArchivedF64` (from rend) only implements `PartialOrd`, not `Ord`.
// We provide manual impls so that `BTreeSet<SortedSetData>` can implement `Archive`.
impl PartialEq for ArchivedSortedSetData {
    fn eq(&self, other: &Self) -> bool {
        self.member.as_slice() == other.member.as_slice()
            && self.score == other.score
    }
}

impl Eq for ArchivedSortedSetData {}

impl Ord for ArchivedSortedSetData {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.score.partial_cmp(&other.score) {
            Some(ordering) => {
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            None => return Ordering::Less,
        }
        self.member.as_slice().cmp(other.member.as_slice())
    }
}

impl PartialOrd for ArchivedSortedSetData {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
