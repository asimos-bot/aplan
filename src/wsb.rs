use std::collections::HashMap;

use crate::{task::Task, task_id::TaskId};
use std::io::Write;

#[derive(Debug)]
pub struct WSB {
    tree: HashMap<TaskId, Task>,
}

impl WSB {

    fn get_root_id() -> TaskId {
        TaskId::new(vec![])
    }

    pub fn new(name: &str) -> Self {
        let root_id = Self::get_root_id();
        let root_task = Task::new(root_id.clone(), name);
        let mut map = HashMap::new();
        map.insert(root_id.clone(), root_task);
        Self {
            tree: map,
        }
    }

    pub fn get_planned_value(&self) -> f64 {
        self.tree.get(&Self::get_root_id()).unwrap().get_planned_value()
    }

    pub fn get_actual_cost(&self) -> f64 {
        self.tree.get(&Self::get_root_id()).unwrap().get_actual_cost()
    }

    pub fn get_task(&self, id: &str) -> Option<&Task> {
        let task_id = TaskId::parse(id)?;
        self.tree.get(&task_id)
    }

    pub fn get_task_mut(&mut self, id: &str) -> Option<&mut Task> {
        let task_id = TaskId::parse(id)?;
        self.tree.get_mut(&task_id)
    }

    pub fn add_task(&mut self, parent_id: &str, name: &str) -> Option<&mut Task> {
        // get parent
        let parent_task_id = TaskId::parse(parent_id)?;
        let parent_task = self.tree.get_mut(&parent_task_id)?;

        // increase number of children
        parent_task.num_child += 1;

        // get new task id
        let mut task_id_vec = parent_task_id.as_vec().clone();
        task_id_vec.push(parent_task.num_child);
        let task_id = TaskId::new(task_id_vec);

        // create task
        let task = Task::new(task_id.clone(), name);

        // add task to task map
        self.tree.insert(task_id.clone(), task);

        self.tree.get_mut(&task_id)
    }

    pub fn expand<const N: usize>(&mut self, arr: &[(&str, &str); N]) -> Option<&mut Self> {
        for (parent_id, task_name) in arr {
            self.add_task(parent_id, task_name)?;
        }
        Some(self)
    }

    fn apply_along_path<F: Fn(&mut Task)>(&mut self, id: &TaskId, func: F) -> Option<()> {
        let root = self.tree.get_mut(&Self::get_root_id())?;
        func(root);
        if &Self::get_root_id() == id {
            return Some(());
        }
        // start iterating from the root's children
        id.as_vec().iter().enumerate().for_each(|(depth, _)| {
            // for each node, get the child associated with the id
            let mut child_id_vec = id.as_vec().clone();
            child_id_vec.truncate(depth+1);
            let child_id = TaskId::new(child_id_vec);
            let child = self.tree.get_mut(&child_id).unwrap();
            func(child);
        });
        Some(())
    }

    pub fn subtract_id(&mut self, child_id: &TaskId, layer_idx: usize) {
        let num_child = self.tree.get(child_id).unwrap().num_child;
        let old_task_id = child_id.clone();
        let mut new_task_id = child_id.clone();
        new_task_id.as_vec_mut()[layer_idx] -= 1;
        let mut task = self.tree.remove(&old_task_id).unwrap();
        task.id = new_task_id.clone();
        self.tree.insert(
            new_task_id,
            task
        );

        child_id.child_ids(num_child).iter().for_each(|node_id| {
            self.subtract_id(node_id, layer_idx)
        })
    }

    pub fn remove(&mut self, id: &str) -> Option<Task> {
        let mut task_id = TaskId::parse(id)?;

        // don't remove if this is a trunk node
        if self.tree.get(&task_id)?.num_child > 0 {
            return None;
        }

        let parent_id = task_id.parent()?;
        let parent_childs = {
            let mut parent = self.tree.get_mut(&parent_id)?;
            let ids = parent.child_ids();
            parent.num_child -= 1;
            ids
        };

        let layer_idx = task_id.as_vec().len() - 1;
        let child_idx = (*task_id.as_vec().last()? as usize) - 1;

        let task = self.tree.remove(&task_id)?;

        // change id of child that comes after id node
        parent_childs.iter().enumerate().for_each(|(index, child_id)| {
            if child_idx < index {
                self.subtract_id(child_id, layer_idx);
            }
        });

        // remove last id child from the parent
        task_id.as_vec_mut()[layer_idx] = parent_childs.len() as u32;
        self.tree.remove(&task_id);

        self.remove_task_stats_from_tree(&task);

        Some(task)
    }

    fn remove_task_stats_from_tree(&mut self, task: &Task) {

        let parent_id = task.id().parent().unwrap();

        // remove planned value
        let planned_value_to_remove = task.clone().get_planned_value();
        self.apply_along_path(&parent_id, |mut task| {
            task.planned_value -= planned_value_to_remove
        });

        // remove actual cost
        let actual_cost_to_remove = task.clone().get_actual_cost();
        self.apply_along_path(&parent_id, |mut task| {
            task.actual_cost -= actual_cost_to_remove
        });
    }

    pub fn set_actual_cost(&mut self, id: &str, actual_cost: f64) -> Option<()> {
        let task_id = TaskId::parse(id)?;
        let parent_id = task_id.parent()?;
        {
            let task = self.tree.get(&task_id)?;
            // can't set actual cost of trunk node
            if task.num_child > 0 {
                return None;
            }
        }
        let old_actual_cost = self.tree.get_mut(&task_id)?.actual_cost;
        self.tree.get_mut(&task_id)?.actual_cost = actual_cost;
        let diff = actual_cost - old_actual_cost;

        self.apply_along_path(&parent_id, |mut task| {
            task.actual_cost += diff;
        })
    }

    pub fn set_planned_value(&mut self, id: &str, planned_value: f64) -> Option<()> {
        let task_id = TaskId::parse(id)?;
        let parent_id = task_id.parent()?;
        {
            let task = self.tree.get(&task_id)?;
            // can't set actual cost of trunk node
            if task.num_child > 0 {
                return None;
            }
        }
        let old_planned_value = self.tree.get_mut(&task_id)?.planned_value;
        self.tree.get_mut(&task_id)?.planned_value = planned_value;
        let diff = planned_value - old_planned_value;

        self.apply_along_path(&parent_id, |mut task| {
            task.planned_value += diff;
        })
    }

    fn subtree_to_dot_str(&self, root_id: &TaskId) -> String {
        let mut s = String::new();
        let root = self.tree.get(root_id).unwrap();
        let root_str = root.to_string();

        root.child_ids().iter().for_each(|child_id| {
            dbg!(&child_id);
            let child = self.tree.get(child_id).unwrap();
            s += &format!("\t\"{}\" -> \"{}\"\n", root_str, child.to_string());
            s += &self.subtree_to_dot_str(child_id);
        });
        s
    }

    pub fn to_dot_str(&self) -> String {
        "digraph G {\n".to_string() +
            &self.subtree_to_dot_str(&Self::get_root_id()) +
            &"}".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks() {
        let mut wsb = WSB::new("Project");

        assert!(wsb.add_task("1", "Create WSB").is_none());
        assert_eq!(wsb.add_task("", "Create WSB"), Some(&mut Task::new(TaskId::new(vec![1]), "Create WSB")));
        assert_eq!(wsb.add_task("1", "Create Task struct"), Some(&mut Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        wsb.expand(&[
            ("", "Create CLI tool"),
                ("2", "Create argument parser"),
                ("2", "Create help menu"),
            ("", "Create GUI tool"),
                ("3", "Create plot visualizer")
        ]);
        assert_eq!(wsb.get_task("1"), Some(&Task::new(TaskId::new(vec![1]), "Create WSB")));
        assert_eq!(wsb.get_task_mut("1"), Some(&mut Task::new(TaskId::new(vec![1]), "Create WSB")));

        assert_eq!(wsb.get_task("1.1"), Some(&Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        assert_eq!(wsb.get_task_mut("1.1"), Some(&mut Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        assert_eq!(wsb.set_planned_value("1.1", 2.0), Some(()));
        assert_eq!(wsb.get_planned_value(), 2.0);
        assert_eq!(wsb.get_task("1.1").unwrap().get_planned_value(), 2.0);
        assert_eq!(wsb.get_task("1").unwrap().get_planned_value(), 2.0);

        assert_eq!(wsb.get_task("2"), Some(&Task::new(TaskId::new(vec![2]), "Create CLI tool")));
        assert_eq!(wsb.get_task_mut("2"), Some(&mut Task::new(TaskId::new(vec![2]), "Create CLI tool")));

        assert_eq!(wsb.get_task("2.1"), Some(&Task::new(TaskId::new(vec![2,1]), "Create argument parser")));
        assert_eq!(wsb.get_task_mut("2.1"), Some(&mut Task::new(TaskId::new(vec![2,1]), "Create argument parser")));
        assert_eq!(wsb.set_planned_value("2.1", 7.0), Some(()));
        assert_eq!(wsb.get_planned_value(), 9.0);
        assert_eq!(wsb.get_task("2.1").unwrap().get_planned_value(), 7.0);
        assert_eq!(wsb.get_task("2.2").unwrap().get_planned_value(), 0.0);
        assert_eq!(wsb.get_task("2").unwrap().get_planned_value(), 7.0);

        assert_eq!(wsb.get_task("2.2"), Some(&Task::new(TaskId::new(vec![2,2]), "Create help menu")));
        assert_eq!(wsb.get_task_mut("2.2"), Some(&mut Task::new(TaskId::new(vec![2,2]), "Create help menu")));
        assert_eq!(wsb.set_planned_value("2.2", 33.0), Some(()));
        assert_eq!(wsb.get_planned_value(), 42.0);
        assert_eq!(wsb.get_task("2.1").unwrap().get_planned_value(), 7.0);
        assert_eq!(wsb.get_task("2.2").unwrap().get_planned_value(), 33.0);
        assert_eq!(wsb.get_task("2").unwrap().get_planned_value(), 40.0);

        assert_eq!(wsb.get_task("3"), Some(&Task::new(TaskId::new(vec![3]), "Create GUI tool")));
        assert_eq!(wsb.get_task_mut("3"), Some(&mut Task::new(TaskId::new(vec![3]), "Create GUI tool")));

        assert_eq!(wsb.get_task("3.1"), Some(&Task::new(TaskId::new(vec![3,1]), "Create plot visualizer")));
        assert_eq!(wsb.get_task_mut("3.1"), Some(&mut Task::new(TaskId::new(vec![3,1]), "Create plot visualizer")));
        assert_eq!(wsb.set_planned_value("3.1", 20.0), Some(()));
        assert_eq!(wsb.get_planned_value(), 62.0);
        assert_eq!(wsb.get_task("3.1").unwrap().get_planned_value(), 20.0);
        assert_eq!(wsb.get_task("3").unwrap().get_planned_value(), 20.0);
        assert_eq!(wsb.remove("2.1"), Some(Task::new(TaskId::new(vec![2,1]), "Create argument parser")));

        assert_eq!(wsb.get_planned_value(), 55.0);
        assert_eq!(wsb.get_task("2.1"), Some(&Task::new(TaskId::new(vec![2, 1]), "Create help menu")));
        assert_eq!(wsb.get_task("2"), Some(&Task::new(TaskId::new(vec![2]), "Create CLI tool")));
        assert_eq!(wsb.get_task("2").unwrap().get_planned_value(), 33.0);

        assert_eq!(wsb.remove("2"), None);
        assert_eq!(wsb.get_planned_value(), 55.0);
        assert_eq!(wsb.remove("2.1"), Some(Task::new(TaskId::new(vec![2,1]), "Create help menu")));
        assert_eq!(wsb.get_planned_value(), 22.0);
        assert_eq!(wsb.get_task("2").unwrap().get_planned_value(), 0.0);
        assert_eq!(wsb.remove("2"), Some(Task::new(TaskId::new(vec![2]), "Create CLI tool")));
        assert_eq!(wsb.get_planned_value(), 22.0);

        assert_eq!(wsb.get_task("1"), Some(&Task::new(TaskId::new(vec![1]), "Create WSB")));
        assert_eq!(wsb.get_task_mut("1"), Some(&mut Task::new(TaskId::new(vec![1]), "Create WSB")));

        assert_eq!(wsb.get_task("1.1"), Some(&Task::new(TaskId::new(vec![1,1]), "Create Task struct")));
        assert_eq!(wsb.get_task_mut("1.1"), Some(&mut Task::new(TaskId::new(vec![1,1]), "Create Task struct")));

        assert_eq!(wsb.get_task("2"), Some(&Task::new(TaskId::new(vec![2]), "Create GUI tool")));
        assert_eq!(wsb.get_task_mut("2"), Some(&mut Task::new(TaskId::new(vec![2]), "Create GUI tool")));

        assert_eq!(wsb.get_task("2.1"), Some(&Task::new(TaskId::new(vec![2,1]), "Create plot visualizer")));
        assert_eq!(wsb.get_task_mut("2.1"), Some(&mut Task::new(TaskId::new(vec![2,1]), "Create plot visualizer")));
        // write!(std::fs::File::create("test").unwrap(), "{}", wsb.to_dot_str());
    }
}
