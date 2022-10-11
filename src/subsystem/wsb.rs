use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::task::{Task, TaskStatus};
use crate::task::task_id::TaskId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct WSB {}

impl WSB {
    pub(crate) fn new(name: &str, map: &mut HashMap<TaskId, Task>) -> Self {
        let root_id = TaskId::get_root_id();
        let root_task = Task::new(root_id.clone(), name);
        map.insert(root_id, root_task);
        Self {}
    }

    /// SAFETY: uses `unwrap` instead of returning an error because a root node should always
    /// exists
    pub(crate) fn name<'a>(&'a self, tasks: &'a HashMap<TaskId, Task>) -> &str {
        tasks.get(&TaskId::get_root_id()).unwrap().name()
    }

    /// SAFETY: uses `unwrap` instead of returning an error because a root node should always
    /// exists
    pub(crate) fn planned_value(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        tasks.get(&TaskId::get_root_id()).unwrap().get_planned_value()
    }

    /// SAFETY: uses `unwrap` instead of returning an error because a root node should always
    /// exists
    pub(crate) fn actual_cost(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        tasks.get(&TaskId::get_root_id()).unwrap().get_actual_cost()
    }

    pub(crate) fn completion_percentage(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        self.done_tasks(tasks).count() as f64 / tasks.len() as f64
    }

    pub(crate) fn earned_value(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        self.planned_value(tasks) * self.completion_percentage(tasks)
    }

    pub(crate) fn spi(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        let res = self.earned_value(tasks) / self.planned_value(tasks);
        if res.is_nan() {
            0.0
        } else {
            res
        }
    }

    pub(crate) fn sv(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        self.earned_value(tasks) - self.planned_value(tasks)
    }

    pub(crate) fn cpi(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        let res = self.earned_value(tasks) / self.actual_cost(tasks);
        if res.is_nan() {
            0.0
        } else {
            res
        }
    }

    pub(crate) fn cv(&self, tasks: &HashMap<TaskId, Task>) -> f64 {
        self.earned_value(tasks) - self.actual_cost(tasks)
    }

    pub(crate) fn get_task<'a>(&'a self, task_id: &TaskId, tasks: &'a HashMap<TaskId, Task>) -> Result<&Task, Error> {
        tasks.get(&task_id).ok_or_else(|| Error::TaskNotFound(task_id.clone()))
    }

    pub(crate) fn get_task_mut<'a>(&'a mut self, task_id: &TaskId, tasks: &'a mut HashMap<TaskId, Task>) -> Result<&mut Task, Error> {
        tasks.get_mut(&task_id).ok_or_else(|| Error::TaskNotFound(task_id.clone()))
    }

    pub(crate) fn next_sibling<'a>(&'a self, task_id: &TaskId, tasks: &'a HashMap<TaskId, Task>) -> Result<&Task, Error> {
        let next_sibling_id = task_id.next_sibling()?;
        self.get_task(&next_sibling_id, tasks)
            .map_err(|_| Error::NoNextSibling(task_id.clone()))
    }

    pub(crate) fn prev_sibling<'a>(&'a self, task_id: &TaskId, tasks: &'a HashMap<TaskId, Task>) -> Result<&Task, Error> {
        let prev_sibling_id = task_id.prev_sibling()?;
        self.get_task(&prev_sibling_id, tasks)
            .map_err(|_| Error::NoPrevSibling(task_id.clone()))
    }

    pub(crate) fn add_task<'a>(&'a mut self, parent_task_id: TaskId, name: &str, tasks: &'a mut HashMap<TaskId, Task>) -> Result<&mut Task, Error> {
        // get parent
        let parent_task = self.get_task_mut(&parent_task_id, tasks)?;

        // increase number of children
        parent_task.num_child += 1;

        // get new task id
        let task_id = parent_task_id.new_child_id(parent_task.num_child)?;

        // create task
        let task = Task::new(task_id.clone(), name);

        // add task to task map
        tasks.insert(task_id.clone(), task);

        // since new tasks are always not done, all parents must be not done too
        self.apply_along_path(&task_id, |task| {
            task.status = TaskStatus::InProgress;
        }, tasks)?;

        self.get_task_mut(&task_id, tasks)
    }

    pub(crate) fn assign_task_to_member<'a>(&'a mut self, task_id: &TaskId, name: &str, tasks: &'a mut HashMap<TaskId, Task>) -> Result<(), Error> {
        if self.get_task(task_id, tasks)?.is_trunk() {
            return Err(Error::TrunkCannotAddMember(task_id.clone()))
        }

        self.apply_along_path(task_id, |task| {
            task.add_member(name)
        }, tasks)
    }

    pub(crate) fn remove_member_from_task<'a>(&'a mut self, task_id: &TaskId, name: &str, tasks: &'a mut HashMap<TaskId, Task>) -> Result<(), Error> {
        let task = self.get_task(task_id, tasks)?;
        if task.is_trunk() {
            return Err(Error::TrunkCannotRemoveMember(task_id.clone()))
        } else if !task.has_member(name) {
            return Err(Error::CannotRemoveMemberFromTask(task_id.clone(), name.to_string()))
        }

        self.apply_along_path(task_id, |task| {
            task.remove_member(name)
        }, tasks)
    }

    pub(crate) fn expand<const N: usize>(&mut self, arr: &[(&str, &str); N], tasks: &mut HashMap<TaskId, Task>) -> Result<&mut Self, Error> {
        for (parent_id, task_name) in arr {
            self.add_task(TaskId::parse(parent_id)?, task_name, tasks)?;
        }
        Ok(self)
    }

    fn apply_along_path<F: Fn(&mut Task)>(&mut self, id: &TaskId, func: F, tasks: &mut HashMap<TaskId, Task>) -> Result<(), Error> {
        id
            .path()
            .try_for_each(|id| {
                let child = self.get_task_mut(&id, tasks)?;
                func(child);
                Ok(())
            })
    }

    fn subtract_id(&mut self, child_id: &TaskId, layer_idx: usize, tasks: &mut HashMap<TaskId, Task>) -> Result<(), Error> {
        let num_child = self.get_task(child_id, tasks)?.num_child;
        let old_task_id = child_id.clone();
        let mut new_task_id = child_id.clone();
        new_task_id.as_vec_mut()[layer_idx] -= 1;
        let mut task = tasks.remove(&old_task_id).ok_or_else(|| Error::TaskNotFound(old_task_id.clone()))?;
        task.id = new_task_id.clone();
        tasks.insert(
            new_task_id,
            task
        );

        child_id.child_ids(num_child).try_for_each(|node_id| {
            self.subtract_id(&node_id, layer_idx, tasks)
        })
    }

    pub(crate) fn remove(&mut self, task_id: &TaskId, tasks: &mut HashMap<TaskId, Task>) -> Result<Task, Error> {
        // don't remove if this is a trunk node
        let mut task_id = task_id.clone();
        if self.get_task(&task_id, tasks)?.num_child > 0 {
            return Err(Error::TrunkCannotBeRemoved(task_id.clone()));
        }

        self.remove_task_stats_from_tasks(&task_id, tasks)?;

        let parent_id = task_id.parent()?;
        let parent_childs: _ = {
            let mut parent = self.get_task_mut(&parent_id, tasks)?;
            parent.num_child -= 1;
            parent.id().child_ids(parent.num_child+1)
                .collect::<Vec<TaskId>>()
        };

        let layer_idx = task_id.len() - 1;
        let child_idx = task_id.child_idx()? as usize - 1;

        let task = tasks.remove(&task_id).ok_or_else(||Error::TaskNotFound(task_id.clone()))?;

        // change id of child that comes after id node
        parent_childs.iter().enumerate().try_for_each(|(index, child_id)| -> Result<(), _> {
            if child_idx < index {
                self.subtract_id(&child_id, layer_idx, tasks)?;
            }
            Ok(())
        })?;

        // remove last id child from the parent
        task_id.as_vec_mut()[layer_idx] = parent_childs.len() as u32;
        tasks.remove(&task_id);

        Ok(task)
    }

    fn remove_task_stats_from_tasks(&mut self, task_id: &TaskId, tasks: &mut HashMap<TaskId, Task>) -> Result<(), Error> {

        self.set_actual_cost(&task_id, 0.0, tasks)?;
        self.set_planned_value(&task_id, 0.0, tasks)?;
        Ok(())
    }

    fn children_are_done(&self, task_id: &TaskId, tasks: &HashMap<TaskId, Task>) -> bool {
        tasks.get(task_id).unwrap()
            .child_ids()
            .find(|id| tasks.get(id).unwrap().status != TaskStatus::Done)
            .is_none()
    }

    pub(crate) fn set_actual_cost(&mut self, task_id: &TaskId, actual_cost: f64, tasks: &mut HashMap<TaskId, Task>) -> Result<(), Error> {
        let parent_id = task_id.parent()?;
        {
            let mut task = self.get_task_mut(&task_id, tasks)?;
            if task.is_trunk() {
                return Err(Error::TrunkCannotChangeCost(task_id.clone()));
            }
            let old_actual_cost = task.actual_cost;
            task.actual_cost = actual_cost;
            let diff = actual_cost - old_actual_cost;

                self.apply_along_path(&parent_id, |mut task| {
                    task.actual_cost += diff;
                }, tasks)?;
        }

        task_id
            .clone()
            .path()
            .rev()
            .try_for_each(|id| {
                if self.children_are_done(&id, tasks) {
                    self.get_task_mut(&id, tasks)?.status = TaskStatus::Done;
                }
                Ok(())
            })
    }

    pub(crate) fn set_planned_value(&mut self, task_id: &TaskId, planned_value: f64, tasks: &mut HashMap<TaskId, Task>) -> Result<(), Error> {
        let parent_id = task_id.parent()?;
        let mut task = self.get_task_mut(&task_id, tasks)?;
        // can't set actual cost of trunk node
        if task.is_trunk() {
            return Err(Error::TrunkCannotChangeValue(task_id.clone()));
        }
        let old_planned_value = task.planned_value;
        task.planned_value = planned_value;
        let diff = planned_value - old_planned_value;

        self.apply_along_path(&parent_id, |mut task| {
            task.planned_value += diff;
        }, tasks)
    }

    pub(crate) fn to_dot_str(&self, tasks: &HashMap<TaskId, Task>) -> String {
        let stats = format!(
            "earned value: {}, spi: {}, sv: {}, cpi: {}, cv: {}",
            self.earned_value(tasks),
            self.spi(tasks),
            self.sv(tasks),
            self.cpi(tasks),
            self.cv(tasks));
        format!(
            "digraph G {{\nlabel=\"{}\"\n{}}}",
            stats,
            self.subtasks_to_dot_str(&TaskId::get_root_id(), tasks))
    }

    fn subtasks_to_dot_str(&self, root_id: &TaskId, tasks: &HashMap<TaskId, Task>) -> String {
        let mut s = String::new();
        let root = tasks.get(root_id).unwrap();
        let root_str = root.to_string();

        root.child_ids().for_each(|child_id| {
            let child = tasks.get(&child_id).unwrap();
            s += &format!("\t\"{}\" -> \"{}\"\n", root_str, child.to_string());
            s += &self.subtasks_to_dot_str(&child_id, tasks);
        });
        s
    }

    fn subtasks_to_tree_str(&self, root_id: &TaskId, prefix: &str, tasks: &HashMap<TaskId, Task>) -> String {
        let mut s = String::new();
        let root = tasks.get(root_id).unwrap();

        root.child_ids().for_each(|child_id| {
            let child = tasks.get(&child_id).unwrap();

            match self.next_sibling(&child_id, tasks) {
                Ok(_) => {
                    s += &format!("{}├─ {}\n", prefix, child);
                    s += &self.subtasks_to_tree_str(&child_id, &format!("{}│  ", prefix), tasks);
                },
                Err(_) => {
                    s += &format!("{}└─ {}\n", prefix, child);
                    s += &self.subtasks_to_tree_str(&child_id, &format!("{}   ", prefix), tasks);
                }
            }
        });
        s
    }

    pub(crate) fn to_tree_str(&self, tasks: &HashMap<TaskId, Task>) -> String {
        let root_id = &TaskId::get_root_id();
        let root = tasks.get(root_id).unwrap();
        format!(
            "{}\n{}",
            root,
            self.subtasks_to_tree_str(&TaskId::get_root_id(), "", tasks))
    }

    pub(crate) fn tasks<'a>(&'a self, tasks: &'a HashMap<TaskId, Task>) -> impl Iterator<Item=&Task> {
        tasks
            .values()
            .filter(|task| task.is_leaf())
    }

    pub(crate) fn todo_tasks<'a>(&'a self, tasks: &'a HashMap<TaskId, Task>) -> impl Iterator<Item=&Task> {
        tasks
            .values()
            .filter(|task| task.is_leaf() && task.status != TaskStatus::Done)
    }

    pub(crate) fn in_progress_tasks<'a>(&'a self, tasks: &'a HashMap<TaskId, Task>) -> impl Iterator<Item=&Task> {
        tasks
            .values()
            .filter(|task| task.is_leaf() && task.status == TaskStatus::InProgress)
    }

    pub(crate) fn done_tasks<'a>(&'a self, tasks: &'a HashMap<TaskId, Task>) -> impl Iterator<Item=&Task> {
        tasks
            .values()
            .filter(|task| task.is_leaf() && task.status == TaskStatus::Done)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn tasks() {
        let mut tasks = HashMap::new();
        let map = &mut tasks;
        let mut wsb = WSB::new("Project", map);

        let root = TaskId::get_root_id();
        let task_id_1 = TaskId::new(vec![1]);
        let task_id_2 = TaskId::new(vec![2]);
        let task_id_3 = TaskId::new(vec![3]);
        let task_id_1_1 = TaskId::new(vec![1, 1]);
        let task_id_2_1 = TaskId::new(vec![2, 1]);
        let task_id_2_2 = TaskId::new(vec![2, 2]);
        let task_id_3_1 = TaskId::new(vec![3, 1]);

        assert!(wsb.add_task(task_id_1.clone(), "Create WSB", map).is_err());
        assert_eq!(wsb.add_task(root.clone(), "Create WSB", map), Ok(&mut Task::new(TaskId::new(vec![1]), "Create WSB")));
        assert_eq!(wsb.add_task(task_id_1.clone(), "Create Task struct", map), Ok(&mut Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        wsb.expand(&[
            ("", "Create CLI tool"),
                ("2", "Create argument parser"),
                ("2", "Create help menu"),
            ("", "Create GUI tool"),
                ("3", "Create plot visualizer")
        ], map).unwrap();
        assert_eq!(wsb.get_task(&task_id_1, map), Ok(&Task::new(TaskId::new(vec![1]), "Create WSB")));
        assert_eq!(wsb.get_task_mut(&task_id_1, map), Ok(&mut Task::new(TaskId::new(vec![1]), "Create WSB")));

        assert_eq!(wsb.get_task(&task_id_1_1, map), Ok(&Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        assert_eq!(wsb.get_task_mut(&task_id_1_1, map), Ok(&mut Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        assert_eq!(wsb.set_planned_value(&task_id_1_1, 2.0, map), Ok(()));
        assert_eq!(wsb.planned_value(map), 2.0);
        assert_eq!(wsb.get_task(&task_id_1_1, map).unwrap().get_planned_value(), 2.0);
        assert_eq!(wsb.get_task(&task_id_1, map).unwrap().get_planned_value(), 2.0);

        assert_eq!(wsb.get_task(&task_id_2, map), Ok(&Task::new(TaskId::new(vec![2]), "Create CLI tool")));
        assert_eq!(wsb.get_task_mut(&task_id_2, map), Ok(&mut Task::new(TaskId::new(vec![2]), "Create CLI tool")));

        assert_eq!(wsb.get_task(&task_id_2_1, map), Ok(&Task::new(TaskId::new(vec![2,1]), "Create argument parser")));
        assert_eq!(wsb.get_task_mut(&task_id_2_1, map), Ok(&mut Task::new(TaskId::new(vec![2,1]), "Create argument parser")));
        assert_eq!(wsb.set_planned_value(&task_id_2_1, 7.0, map), Ok(()));
        assert_eq!(wsb.planned_value(map), 9.0);
        assert_eq!(wsb.get_task(&task_id_2_1, map).unwrap().get_planned_value(), 7.0);
        assert_eq!(wsb.get_task(&task_id_2_2, map).unwrap().get_planned_value(), 0.0);
        assert_eq!(wsb.get_task(&task_id_2, map).unwrap().get_planned_value(), 7.0);

        assert_eq!(wsb.get_task(&task_id_2_2, map), Ok(&Task::new(TaskId::new(vec![2,2]), "Create help menu")));
        assert_eq!(wsb.get_task_mut(&task_id_2_2, map), Ok(&mut Task::new(TaskId::new(vec![2,2]), "Create help menu")));
        assert_eq!(wsb.set_planned_value(&task_id_2_2, 33.0, map), Ok(()));
        assert_eq!(wsb.planned_value(map), 42.0);
        assert_eq!(wsb.get_task(&task_id_2_1, map).unwrap().get_planned_value(), 7.0);
        assert_eq!(wsb.get_task(&task_id_2_2, map).unwrap().get_planned_value(), 33.0);
        assert_eq!(wsb.get_task(&task_id_2, map).unwrap().get_planned_value(), 40.0);

        assert_eq!(wsb.get_task(&task_id_3, map), Ok(&Task::new(TaskId::new(vec![3]), "Create GUI tool")));
        assert_eq!(wsb.get_task_mut(&task_id_3, map), Ok(&mut Task::new(TaskId::new(vec![3]), "Create GUI tool")));

        assert_eq!(wsb.get_task(&task_id_3_1, map), Ok(&Task::new(TaskId::new(vec![3,1]), "Create plot visualizer")));
        assert_eq!(wsb.get_task_mut(&task_id_3_1, map), Ok(&mut Task::new(TaskId::new(vec![3,1]), "Create plot visualizer")));
        assert_eq!(wsb.set_planned_value(&task_id_3_1, 20.0, map), Ok(()));
        assert_eq!(wsb.planned_value(map), 62.0);
        assert_eq!(wsb.get_task(&task_id_3_1, map).unwrap().get_planned_value(), 20.0);
        assert_eq!(wsb.get_task(&task_id_3, map).unwrap().get_planned_value(), 20.0);
        assert_eq!(wsb.remove(&task_id_2_1, map), Ok(Task::new(TaskId::new(vec![2,1]), "Create argument parser")));

        assert_eq!(wsb.planned_value(map), 55.0);
        assert_eq!(wsb.get_task(&task_id_2_1, map), Ok(&Task::new(TaskId::new(vec![2, 1]), "Create help menu")));
        assert_eq!(wsb.get_task(&task_id_2, map), Ok(&Task::new(TaskId::new(vec![2]), "Create CLI tool")));
        assert_eq!(wsb.get_task(&task_id_2, map).unwrap().get_planned_value(), 33.0);

        assert_eq!(wsb.remove(&task_id_2, map), Err(Error::TrunkCannotBeRemoved(task_id_2.clone())));
        assert_eq!(wsb.planned_value(map), 55.0);
        assert_eq!(wsb.remove(&task_id_2_1, map), Ok(Task::new(TaskId::new(vec![2,1]), "Create help menu")));
        assert_eq!(wsb.planned_value(map), 22.0);
        assert_eq!(wsb.get_task(&task_id_2, map).unwrap().get_planned_value(), 0.0);
        assert_eq!(wsb.remove(&task_id_2, map), Ok(Task::new(TaskId::new(vec![2]), "Create CLI tool")));
        assert_eq!(wsb.planned_value(map), 22.0);

        assert_eq!(wsb.get_task(&task_id_1, map), Ok(&Task::new(TaskId::new(vec![1]), "Create WSB")));
        assert_eq!(wsb.get_task_mut(&task_id_1, map), Ok(&mut Task::new(TaskId::new(vec![1]), "Create WSB")));

        assert_eq!(wsb.get_task(&task_id_1_1, map), Ok(&Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        assert_eq!(wsb.get_task_mut(&task_id_1_1, map), Ok(&mut Task::new(TaskId::new(vec![1,1]), "Create Task struct")));

        assert_eq!(wsb.get_task(&task_id_2, map), Ok(&Task::new(TaskId::new(vec![2]), "Create GUI tool")));
        assert_eq!(wsb.get_task_mut(&task_id_2, map), Ok(&mut Task::new(TaskId::new(vec![2]), "Create GUI tool")));

        assert_eq!(wsb.get_task(&task_id_2_1, map), Ok(&Task::new(TaskId::new(vec![2,1]), "Create plot visualizer")));
        assert_eq!(wsb.get_task_mut(&task_id_2_1, map), Ok(&mut Task::new(TaskId::new(vec![2,1]), "Create plot visualizer")));
    }
}
