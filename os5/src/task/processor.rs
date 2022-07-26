//! Implementation of [`Processor`] and Intersection of control& flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.


use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;
use crate::timer::get_time_ms;
use crate::config::MAX_SYSCALL_NUM;

/// Processor management structure
pub struct Processor {
    /// The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,
    /// The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|task| Arc::clone(task))
    }
}

lazy_static! {
    /// PROCESSOR instance through lazy_static!
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

/// The main part of process execution and scheduling
///
/// Loop fetch_task to get the process that needs to run,
/// and switch the process through __switch
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let k = task.clone();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            //info!("switch to {:?} in run tasks", k.pid.0);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
                //info!("switch ok....")
            }
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get token of the address space of current task
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

/// Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

/// Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

pub fn get_current_task_status() -> TaskStatus {
    TaskStatus::Running
}

pub fn get_current_task_costed_time() -> usize {
    let task = current_task().unwrap();
    let now = get_time_ms();
    let first_time = task.inner_exclusive_access().first_time;
    info!("task {:?} now time is {:?}", task.pid.0, now);
    info!("task {:?} first time is {:?}", task.pid.0, first_time);

    let costs = now - first_time ;
    info!("task {:?} cost time {:?}",task.pid.0, costs);
    costs

}

pub fn add_one_to_current_task(call_id: usize)  {
    let task = current_task().unwrap();
    
    task.inner_exclusive_access().syscall_times[call_id] += 1;
    //info!("add task {current} syscall {call_id} to {:?}", inner.tasks[current].syscall_times[call_id]);
}

pub fn get_current_task_syscall_times() -> [u32; MAX_SYSCALL_NUM] {
    let task = current_task().unwrap();
    let st = task.inner_exclusive_access().syscall_times.clone();
    st
}

pub fn mmap( start: usize, len: usize, port: usize) -> isize {
    let  task = current_task().unwrap();
    let ret = task.inner_exclusive_access().memory_set.mmap(start, len, port);
    ret
}

pub fn munmap( start: usize, len: usize ) -> isize {
    let task = current_task().unwrap();
    let ret = task.inner_exclusive_access().memory_set.munmap(start, len);
    ret
    
}