mod keyed_state;
mod memory_state;
mod persistent_state;
mod single_state;

use std::borrow::Cow;
use std::ops::Deref;
use std::rc::Rc;
use std::{slice, vec};

use basics::data::SizeOf;
use prelude::*;

pub use self::memory_state::MemoryState;
pub use self::persistent_state::PersistentState;

pub trait State: SizeOf + Send {
    /// Add an index keyed by the given columns and replayed to by the given partial tags.
    fn add_key(&mut self, columns: &[usize], partial: Option<Vec<Tag>>);

    /// Returns whether this state is currently keyed on anything. If not, then it cannot store any
    /// infromation and is thus "not useful".
    fn is_useful(&self) -> bool;

    fn is_partial(&self) -> bool;

    // Inserts or removes each record into State. Records that miss all indices in partial state
    // are removed from `records` (thus the mutable reference).
    fn process_records(&mut self, records: &mut Records, partial_tag: Option<Tag>);

    fn mark_hole(&mut self, key: &[DataType], tag: &Tag);

    fn mark_filled(&mut self, key: Vec<DataType>, tag: &Tag);

    fn lookup<'a>(&'a self, columns: &[usize], key: &KeyType) -> LookupResult<'a>;

    fn rows(&self) -> usize;

    fn keys(&self) -> Vec<Vec<usize>>;

    /// Return a copy of all records. Panics if the state is only partially materialized.
    fn cloned_records(&self) -> Vec<Vec<DataType>>;

    /// Evict `count` randomly selected keys, returning key colunms of the index chosen to evict
    /// from along with the keys evicted and the number of bytes evicted.
    fn evict_random_keys(&mut self, count: usize) -> (&[usize], Vec<Vec<DataType>>, u64);

    /// Evict the listed keys from the materialization targeted by `tag`, returning the key columns
    /// of the index that was evicted from and the number of bytes evicted.
    fn evict_keys(&mut self, tag: &Tag, keys: &[Vec<DataType>]) -> Option<(&[usize], u64)>;
}

#[derive(Clone, Debug)]
pub struct Row(pub(crate) Rc<Vec<DataType>>);

unsafe impl Send for Row {}

impl Deref for Row {
    type Target = Vec<DataType>;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
impl SizeOf for Row {
    fn size_of(&self) -> u64 {
        use std::mem::size_of;
        size_of::<Self>() as u64
    }
    fn deep_size_of(&self) -> u64 {
        (*self.0).deep_size_of()
    }
}

/// An std::borrow::Cow-like wrapper around a collection of rows.
pub enum RecordResult<'a> {
    Borrowed(&'a [Row]),
    Owned(Vec<Vec<DataType>>),
}

impl<'a> RecordResult<'a> {
    pub fn len(&self) -> usize {
        match *self {
            RecordResult::Borrowed(rs) => rs.len(),
            RecordResult::Owned(ref rs) => rs.len(),
        }
    }
}

impl<'a> IntoIterator for RecordResult<'a> {
    type Item = Cow<'a, [DataType]>;
    type IntoIter = RecordResultIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            RecordResult::Borrowed(rs) => RecordResultIterator::Borrowed(rs.into_iter()),
            RecordResult::Owned(rs) => RecordResultIterator::Owned(rs.into_iter()),
        }
    }
}

pub enum RecordResultIterator<'a> {
    Owned(vec::IntoIter<Vec<DataType>>),
    Borrowed(slice::Iter<'a, Row>),
}

impl<'a> Iterator for RecordResultIterator<'a> {
    type Item = Cow<'a, [DataType]>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            RecordResultIterator::Borrowed(iter) => iter.next().map(|r| Cow::from(&r[..])),
            RecordResultIterator::Owned(iter) => iter.next().map(|r| Cow::from(r)),
        }
    }
}

pub enum LookupResult<'a> {
    Some(RecordResult<'a>),
    Missing,
}
