use std::{fmt::Display};

use serde::{Serialize, Deserialize};

use crate::error::Error;

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskId {
    id: Vec<u32>,
}

impl TaskId {

    pub fn new(id: Vec<u32>) -> Self {
        Self { id }
    }

    pub fn as_vec(&self) -> &Vec<u32> {
        &self.id
    }

    pub fn as_vec_mut(&mut self) -> &mut Vec<u32> {
        &mut self.id
    }

    pub fn iter(&self) -> std::slice::Iter<'_, u32> {
        self.id.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> {
        self.id.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.id.len()
    }

    pub fn into_iter(self) -> std::vec::IntoIter<u32> {
        self.id.into_iter()
    }

    pub fn child_idx(&self) -> Result<u32, Error> {
        self.id
            .last()
            .ok_or_else(|| Error::NoChildIndex(self.clone()))
            .cloned()
    }

    pub fn parse(id: &str) -> Result<Self, Error> {
        if id.is_empty() {
            return Ok(TaskId::new(vec![]));
        }
        let vec = id
            .split(".")
            .map(|n| n.parse::<u32>())
            .collect::<Result<Vec<u32>, _>>()
            .or_else(|_| Err(Error::BadTaskIdString(id.to_string())))?;
        Ok(TaskId::new(vec))
    }

    pub fn parent(&self) -> Result<TaskId, Error> {
        let vec_len = self.id.len();
        if vec_len < 1 {
            return Err(Error::NoParent(self.clone()));
        }
        let parent_vec = self.id[..vec_len-1].to_vec();
        Ok(TaskId::new(parent_vec))
    }

    pub(crate) fn prev_sibling(&self) -> Result<TaskId, Error> {
        let parent_id = self.parent()?;
        let child_idx = self.child_idx()?;
        parent_id.new_child_id(child_idx-1)
    }

    pub(crate) fn next_sibling(&self) -> Result<TaskId, Error> {
        let parent_id = self.parent()?;
        let child_idx = self.child_idx()?;
        parent_id.new_child_id(child_idx+1)
    }

    pub fn child_ids(&self, num_childs: u32) -> impl Iterator<Item=TaskId> + '_ {

        let id_vec = self.as_vec();
        (1..num_childs+1).map(|child_index| {
            let mut vec = id_vec.clone();
            vec.push(child_index);
            TaskId::new(vec)
        }).into_iter()
    }

    pub fn path(&self) -> impl Iterator<Item=TaskId> + DoubleEndedIterator + '_ {

        let id_iter = self.id
            .iter()
            .enumerate()
            .map(|(layer_idx, _)| {
                let mut id_vec = self.id.clone();
                id_vec.truncate(layer_idx + 1);
                TaskId::new(id_vec)
            });
        std::iter::once(Self::get_root_id())
            .chain(id_iter)
            .into_iter()
    }

    pub fn new_child_id(&self, child_num: u32) -> Result<TaskId, Error> {
        if child_num == 0 {
            return Err(Error::BadTaskIdNum)
        }
        let id_vec = self
            .as_vec()
            .iter()
            .cloned()
            .chain(std::iter::once(child_num))
            .collect::<Vec<u32>>();
        Ok(TaskId::new(id_vec))
    }

    pub fn get_root_id() -> TaskId {
        TaskId::new(vec![])
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id
               .iter()
               .map(|n| n.to_string())
               .collect::<Vec<String>>()
               .join("."))
    }
}

impl FromIterator<u32> for TaskId {
    fn from_iter<T: IntoIterator<Item = u32>>(iter: T) -> Self {
        TaskId::new(iter.into_iter().collect::<Vec<u32>>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion() {
        let task_id = TaskId::new(vec![1,1]);
        assert_eq!(task_id.as_vec(), &vec![1,1]);
        assert_eq!(task_id.to_string(), "1.1");
        assert_eq!(TaskId::new(vec![1,2,3,4]).to_string(), "1.2.3.4");
    }

    #[test]
    fn parse() {
        let empty_vec : Vec<u32> = vec![];
        assert_eq!(TaskId::parse("").unwrap().as_vec(), &empty_vec);
        assert_eq!(TaskId::parse("1.1").unwrap().as_vec(), &vec![1,1]);
        assert_eq!(TaskId::parse("4.523.123").unwrap().as_vec(), &vec![4, 523, 123]);
        assert!(TaskId::parse(".1.1").is_err());
        assert!(TaskId::parse("1.1.").is_err());
    }

    #[test]
    fn parent_id() {
        assert_eq!(TaskId::parse("1.1").unwrap().parent().unwrap().as_vec(), &vec![1]);
        assert_eq!(TaskId::parse("1.1.234.12").unwrap().parent().unwrap().as_vec(), &vec![1,1,234]);
        assert_eq!(TaskId::parse("2.534.234.12.243.123").unwrap().parent().unwrap().as_vec(), &vec![2, 534, 234, 12, 243]);
    }
}
