//! --- Qanto Native Async Runtime ---
//! v1.0.0 - Custom Async Implementation
//! This module provides a native async runtime for Qanto,
//! replacing tokio with custom implementations.
//!
//! Features:
//! - Custom executor for async tasks
//! - Native TCP/UDP socket handling
//! - Timer and timeout implementations
//! - Channel implementations for message passing
//! - File system operations
//! - Signal handling
//! - Thread pool management

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Condvar};
use std::task::{Context, Poll, Waker};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddr};
use std::io::{self, Read, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QantoAsyncError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Task execution error: {0}")]
    TaskError(String),
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Timeout occurred")]
    Timeout,
    #[error("Runtime shutdown")]
    Shutdown,
}

/// Task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

static NEXT_TASK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl TaskId {
    fn new() -> Self {
        TaskId(NEXT_TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// Task state
#[derive(Debug, Clone)]
enum TaskState {
    Ready,
    Running,
    Waiting,
    Completed,
    Failed(String),
}

/// Async task wrapper
struct Task {
    id: TaskId,
    future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    state: TaskState,
    waker: Option<Waker>,
}

/// Custom executor for async tasks
pub struct QantoExecutor {
    tasks: Arc<Mutex<HashMap<TaskId, Task>>>,
    ready_queue: Arc<Mutex<VecDeque<TaskId>>>,
    worker_threads: Vec<JoinHandle<()>>,
    shutdown: Arc<Mutex<bool>>,
    condvar: Arc<Condvar>,
    thread_count: usize,
}

impl QantoExecutor {
    pub fn new(thread_count: usize) -> Self {
        let tasks = Arc::new(Mutex::new(HashMap::new()));
        let ready_queue = Arc::new(Mutex::new(VecDeque::new()));
        let shutdown = Arc::new(Mutex::new(false));
        let condvar = Arc::new(Condvar::new());
        
        let mut worker_threads = Vec::new();
        
        for i in 0..thread_count {
            let tasks_clone = Arc::clone(&tasks);
            let ready_queue_clone = Arc::clone(&ready_queue);
            let shutdown_clone = Arc::clone(&shutdown);
            let condvar_clone = Arc::clone(&condvar);
            
            let handle = thread::spawn(move || {
                Self::worker_loop(i, tasks_clone, ready_queue_clone, shutdown_clone, condvar_clone);
            });
            
            worker_threads.push(handle);
        }
        
        Self {
            tasks,
            ready_queue,
            worker_threads,
            shutdown,
            condvar,
            thread_count,
        }
    }
    
    /// Spawn a new async task
    pub fn spawn<F>(&self, future: F) -> TaskId
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task_id = TaskId::new();
        let task = Task {
            id: task_id,
            future: Box::pin(future),
            state: TaskState::Ready,
            waker: None,
        };
        
        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(task_id, task);
        }
        
        {
            let mut ready_queue = self.ready_queue.lock().unwrap();
            ready_queue.push_back(task_id);
        }
        
        self.condvar.notify_one();
        task_id
    }
    
    /// Block on a future until completion
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: Future<Output = T>,
    {
        let mut future = Box::pin(future);
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(result) => return result,
                Poll::Pending => {
                    // Yield to other tasks
                    thread::yield_now();
                }
            }
        }
    }
    
    /// Shutdown the executor
    pub fn shutdown(self) {
        {
            let mut shutdown = self.shutdown.lock().unwrap();
            *shutdown = true;
        }
        
        self.condvar.notify_all();
        
        for handle in self.worker_threads {
            handle.join().unwrap();
        }
    }
    
    fn worker_loop(
        worker_id: usize,
        tasks: Arc<Mutex<HashMap<TaskId, Task>>>,
        ready_queue: Arc<Mutex<VecDeque<TaskId>>>,
        shutdown: Arc<Mutex<bool>>,
        condvar: Arc<Condvar>,
    ) {
        println!("[QantoAsync] Worker {} started", worker_id);
        
        loop {
            let task_id = {
                let mut ready_queue = ready_queue.lock().unwrap();
                
                while ready_queue.is_empty() {
                    let shutdown_guard = shutdown.lock().unwrap();
                    if *shutdown_guard {
                        println!("[QantoAsync] Worker {} shutting down", worker_id);
                        return;
                    }
                    drop(shutdown_guard);
                    
                    ready_queue = condvar.wait(ready_queue).unwrap();
                }
                
                ready_queue.pop_front().unwrap()
            };
            
            // Execute the task
            let should_reschedule = {
                let mut tasks = tasks.lock().unwrap();
                if let Some(task) = tasks.get_mut(&task_id) {
                    let waker = task_waker(task_id, Arc::clone(&ready_queue), Arc::clone(&condvar));
                    let mut context = Context::from_waker(&waker);
                    
                    task.state = TaskState::Running;
                    
                    match task.future.as_mut().poll(&mut context) {
                        Poll::Ready(()) => {
                            task.state = TaskState::Completed;
                            false // Task completed, don't reschedule
                        },
                        Poll::Pending => {
                            task.state = TaskState::Waiting;
                            task.waker = Some(waker);
                            false // Task is waiting, don't reschedule immediately
                        }
                    }
                } else {
                    false // Task not found
                }
            };
            
            if should_reschedule {
                let mut ready_queue = ready_queue.lock().unwrap();
                ready_queue.push_back(task_id);
                condvar.notify_one();
            }
        }
    }
}

/// Create a waker for a task
fn task_waker(task_id: TaskId, ready_queue: Arc<Mutex<VecDeque<TaskId>>>, condvar: Arc<Condvar>) -> Waker {
    use std::task::{RawWaker, RawWakerVTable};
    
    let data = Box::into_raw(Box::new((task_id, ready_queue, condvar))) as *const ();
    let vtable = &TASK_WAKER_VTABLE;
    let raw_waker = RawWaker::new(data, vtable);
    unsafe { Waker::from_raw(raw_waker) }
}

const TASK_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    task_waker_clone,
    task_waker_wake,
    task_waker_wake_by_ref,
    task_waker_drop,
);

unsafe fn task_waker_clone(data: *const ()) -> std::task::RawWaker {
    let (task_id, ready_queue, condvar) = &*(data as *const (TaskId, Arc<Mutex<VecDeque<TaskId>>>, Arc<Condvar>));
    let cloned_data = Box::into_raw(Box::new((*task_id, Arc::clone(ready_queue), Arc::clone(condvar)))) as *const ();
    std::task::RawWaker::new(cloned_data, &TASK_WAKER_VTABLE)
}

unsafe fn task_waker_wake(data: *const ()) {
    let (task_id, ready_queue, condvar) = *Box::from_raw(data as *mut (TaskId, Arc<Mutex<VecDeque<TaskId>>>, Arc<Condvar>));
    {
        let mut queue = ready_queue.lock().unwrap();
        queue.push_back(task_id);
    }
    condvar.notify_one();
}

unsafe fn task_waker_wake_by_ref(data: *const ()) {
    let (task_id, ready_queue, condvar) = &*(data as *const (TaskId, Arc<Mutex<VecDeque<TaskId>>>, Arc<Condvar>));
    {
        let mut queue = ready_queue.lock().unwrap();
        queue.push_back(*task_id);
    }
    condvar.notify_one();
}

unsafe fn task_waker_drop(data: *const ()) {
    drop(Box::from_raw(data as *mut (TaskId, Arc<Mutex<VecDeque<TaskId>>>, Arc<Condvar>)));
}

/// No-op waker for blocking operations
fn noop_waker() -> Waker {
    use std::task::{RawWaker, RawWakerVTable};
    
    const NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(std::ptr::null(), &NOOP_VTABLE),
        |_| {},
        |_| {},
        |_| {},
    );
    
    let raw_waker = RawWaker::new(std::ptr::null(), &NOOP_VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

/// Timer implementation
pub struct QantoTimer {
    deadline: Instant,
    completed: bool,
}

impl QantoTimer {
    pub fn new(duration: Duration) -> Self {
        Self {
            deadline: Instant::now() + duration,
            completed: false,
        }
    }
    
    pub fn at(deadline: Instant) -> Self {
        Self {
            deadline,
            completed: false,
        }
    }
}

impl Future for QantoTimer {
    type Output = ();
    
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.completed {
            return Poll::Ready(());
        }
        
        if Instant::now() >= self.deadline {
            self.completed = true;
            Poll::Ready(())
        } else {
            // Schedule wake-up
            let waker = cx.waker().clone();
            let deadline = self.deadline;
            
            thread::spawn(move || {
                let now = Instant::now();
                if deadline > now {
                    thread::sleep(deadline - now);
                }
                waker.wake();
            });
            
            Poll::Pending
        }
    }
}

/// Sleep function
pub async fn sleep(duration: Duration) {
    QantoTimer::new(duration).await
}

/// Timeout wrapper
pub async fn timeout<F, T>(duration: Duration, future: F) -> Result<T, QantoAsyncError>
where
    F: Future<Output = T>,
{
    let timer = QantoTimer::new(duration);
    let mut future = Box::pin(future);
    let mut timer = Box::pin(timer);
    
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return Ok(result),
            Poll::Pending => {}
        }
        
        match timer.as_mut().poll(&mut context) {
            Poll::Ready(()) => return Err(QantoAsyncError::Timeout),
            Poll::Pending => {}
        }
        
        thread::yield_now();
    }
}

/// Channel implementation
pub mod channel {
    use super::*;
    use std::sync::mpsc;
    
    /// Unbounded channel
    pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
        let (tx, rx) = mpsc::channel();
        (
            UnboundedSender { sender: tx },
            UnboundedReceiver { receiver: rx },
        )
    }
    
    /// Bounded channel
    pub fn bounded<T>(capacity: usize) -> (BoundedSender<T>, BoundedReceiver<T>) {
        let (tx, rx) = mpsc::sync_channel(capacity);
        (
            BoundedSender { sender: tx },
            BoundedReceiver { receiver: rx },
        )
    }
    
    /// Unbounded sender
    pub struct UnboundedSender<T> {
        sender: mpsc::Sender<T>,
    }
    
    impl<T> UnboundedSender<T> {
        pub fn send(&self, value: T) -> Result<(), QantoAsyncError> {
            self.sender.send(value).map_err(|_| QantoAsyncError::ChannelClosed)
        }
    }
    
    impl<T> Clone for UnboundedSender<T> {
        fn clone(&self) -> Self {
            Self {
                sender: self.sender.clone(),
            }
        }
    }
    
    /// Unbounded receiver
    pub struct UnboundedReceiver<T> {
        receiver: mpsc::Receiver<T>,
    }
    
    impl<T> UnboundedReceiver<T> {
        pub async fn recv(&mut self) -> Result<T, QantoAsyncError> {
            // Convert blocking recv to async
            let receiver = &self.receiver;
            loop {
                match receiver.try_recv() {
                    Ok(value) => return Ok(value),
                    Err(mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err(QantoAsyncError::ChannelClosed);
                    }
                }
            }
        }
        
        pub fn try_recv(&mut self) -> Result<T, QantoAsyncError> {
            match self.receiver.try_recv() {
                Ok(value) => Ok(value),
                Err(mpsc::TryRecvError::Empty) => Err(QantoAsyncError::ChannelClosed),
                Err(mpsc::TryRecvError::Disconnected) => Err(QantoAsyncError::ChannelClosed),
            }
        }
    }
    
    /// Bounded sender
    pub struct BoundedSender<T> {
        sender: mpsc::SyncSender<T>,
    }
    
    impl<T> BoundedSender<T> {
        pub async fn send(&self, value: T) -> Result<(), QantoAsyncError> {
            // Convert blocking send to async
            loop {
                match self.sender.try_send(value) {
                    Ok(()) => return Ok(()),
                    Err(mpsc::TrySendError::Full(val)) => {
                        sleep(Duration::from_millis(1)).await;
                        return self.send(val).await;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        return Err(QantoAsyncError::ChannelClosed);
                    }
                }
            }
        }
    }
    
    impl<T> Clone for BoundedSender<T> {
        fn clone(&self) -> Self {
            Self {
                sender: self.sender.clone(),
            }
        }
    }
    
    /// Bounded receiver
    pub struct BoundedReceiver<T> {
        receiver: mpsc::Receiver<T>,
    }
    
    impl<T> BoundedReceiver<T> {
        pub async fn recv(&mut self) -> Result<T, QantoAsyncError> {
            // Convert blocking recv to async
            loop {
                match self.receiver.try_recv() {
                    Ok(value) => return Ok(value),
                    Err(mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err(QantoAsyncError::ChannelClosed);
                    }
                }
            }
        }
    }
    
    /// Oneshot channel
    pub fn oneshot<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (
            OneshotSender { sender: Some(tx) },
            OneshotReceiver { receiver: rx },
        )
    }
    
    pub struct OneshotSender<T> {
        sender: Option<mpsc::SyncSender<T>>,
    }
    
    impl<T> OneshotSender<T> {
        pub fn send(mut self, value: T) -> Result<(), T> {
            if let Some(sender) = self.sender.take() {
                sender.send(value).map_err(|mpsc::SendError(val)| val)
            } else {
                Err(value)
            }
        }
    }
    
    pub struct OneshotReceiver<T> {
        receiver: mpsc::Receiver<T>,
    }
    
    impl<T> OneshotReceiver<T> {
        pub async fn recv(self) -> Result<T, QantoAsyncError> {
            loop {
                match self.receiver.try_recv() {
                    Ok(value) => return Ok(value),
                    Err(mpsc::TryRecvError::Empty) => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err(QantoAsyncError::ChannelClosed);
                    }
                }
            }
        }
    }
}

/// TCP networking
pub mod net {
    use super::*;
    
    /// Async TCP listener
    pub struct QantoTcpListener {
        listener: TcpListener,
    }
    
    impl QantoTcpListener {
        pub fn bind(addr: SocketAddr) -> Result<Self, QantoAsyncError> {
            let listener = TcpListener::bind(addr)?;
            listener.set_nonblocking(true)?;
            Ok(Self { listener })
        }
        
        pub async fn accept(&self) -> Result<(QantoTcpStream, SocketAddr), QantoAsyncError> {
            loop {
                match self.listener.accept() {
                    Ok((stream, addr)) => {
                        stream.set_nonblocking(true)?;
                        return Ok((QantoTcpStream { stream }, addr));
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(e) => return Err(QantoAsyncError::Io(e)),
                }
            }
        }
    }
    
    /// Async TCP stream
    pub struct QantoTcpStream {
        stream: TcpStream,
    }
    
    impl QantoTcpStream {
        pub async fn connect(addr: SocketAddr) -> Result<Self, QantoAsyncError> {
            let stream = TcpStream::connect(addr)?;
            stream.set_nonblocking(true)?;
            Ok(Self { stream })
        }
        
        pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, QantoAsyncError> {
            loop {
                match self.stream.read(buf) {
                    Ok(n) => return Ok(n),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(e) => return Err(QantoAsyncError::Io(e)),
                }
            }
        }
        
        pub async fn write(&mut self, buf: &[u8]) -> Result<usize, QantoAsyncError> {
            loop {
                match self.stream.write(buf) {
                    Ok(n) => return Ok(n),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(e) => return Err(QantoAsyncError::Io(e)),
                }
            }
        }
        
        pub async fn write_all(&mut self, mut buf: &[u8]) -> Result<(), QantoAsyncError> {
            while !buf.is_empty() {
                let n = self.write(buf).await?;
                buf = &buf[n..];
            }
            Ok(())
        }
        
        pub fn peer_addr(&self) -> Result<SocketAddr, QantoAsyncError> {
            Ok(self.stream.peer_addr()?)
        }
        
        pub fn local_addr(&self) -> Result<SocketAddr, QantoAsyncError> {
            Ok(self.stream.local_addr()?)
        }
    }
    
    /// Async UDP socket
    pub struct QantoUdpSocket {
        socket: UdpSocket,
    }
    
    impl QantoUdpSocket {
        pub fn bind(addr: SocketAddr) -> Result<Self, QantoAsyncError> {
            let socket = UdpSocket::bind(addr)?;
            socket.set_nonblocking(true)?;
            Ok(Self { socket })
        }
        
        pub async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), QantoAsyncError> {
            loop {
                match self.socket.recv_from(buf) {
                    Ok((n, addr)) => return Ok((n, addr)),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(e) => return Err(QantoAsyncError::Io(e)),
                }
            }
        }
        
        pub async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize, QantoAsyncError> {
            loop {
                match self.socket.send_to(buf, addr) {
                    Ok(n) => return Ok(n),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        sleep(Duration::from_millis(1)).await;
                        continue;
                    }
                    Err(e) => return Err(QantoAsyncError::Io(e)),
                }
            }
        }
    }
}

/// File system operations
pub mod fs {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::thread;
    
    /// Async file operations
    pub async fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String, QantoAsyncError> {
        let path = path.as_ref().to_path_buf();
let (tx, rx) = channel::oneshot();
thread::spawn(move || {
    let result = fs::read_to_string(path).map_err(QantoAsyncError::from);
    tx.send(result).ok();
});
rx.recv().await??.map_err(QantoAsyncError::from)
    }
    
    pub async fn write<P: AsRef<Path>>(path: P, contents: &str) -> Result<(), QantoAsyncError> {
        let path = path.as_ref().to_path_buf();
let contents = contents.to_owned();
let (tx, rx) = channel::oneshot();
thread::spawn(move || {
    let result = fs::write(path, contents).map_err(QantoAsyncError::from);
    tx.send(result).ok();
});
rx.recv().await??.map_err(QantoAsyncError::from)
    }
    
    pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<(), QantoAsyncError> {
        let path = path.as_ref().to_path_buf();
let (tx, rx) = channel::oneshot();
thread::spawn(move || {
    let result = fs::create_dir_all(path).map_err(QantoAsyncError::from);
    tx.send(result).ok();
});
rx.recv().await??.map_err(QantoAsyncError::from)
    }
    
    pub async fn remove_file<P: AsRef<Path>>(path: P) -> Result<(), QantoAsyncError> {
        let path = path.as_ref().to_path_buf();
let (tx, rx) = channel::oneshot();
thread::spawn(move || {
    let result = fs::remove_file(path).map_err(QantoAsyncError::from);
    tx.send(result).ok();
});
rx.recv().await??.map_err(QantoAsyncError::from)
    }
}

/// Process operations
pub mod process {
    use super::*;
    use std::process::{Command, Stdio};
    
    /// Async command execution
    pub struct QantoCommand {
        command: Command,
    }
    
    impl QantoCommand {
        pub fn new(program: &str) -> Self {
            Self {
                command: Command::new(program),
            }
        }
        
        pub fn arg(&mut self, arg: &str) -> &mut Self {
            self.command.arg(arg);
            self
        }
        
        pub fn args<I, S>(&mut self, args: I) -> &mut Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<std::ffi::OsStr>,
        {
            self.command.args(args);
            self
        }
        
        pub fn env(&mut self, key: &str, val: &str) -> &mut Self {
            self.command.env(key, val);
            self
        }
        
        pub fn stdout(&mut self, cfg: Stdio) -> &mut Self {
            self.command.stdout(cfg);
            self
        }
        
        pub fn stderr(&mut self, cfg: Stdio) -> &mut Self {
            self.command.stderr(cfg);
            self
        }
        
        pub async fn output(&mut self) -> Result<std::process::Output, QantoAsyncError> {
            let mut command = self.command;
let (tx, rx) = channel::oneshot();
thread::spawn(move || {
    let result = command.output().map_err(QantoAsyncError::from);
    tx.send(result).ok();
});
rx.recv().await??
        }
        
        pub async fn status(&mut self) -> Result<std::process::ExitStatus, QantoAsyncError> {
            let mut command = self.command;
let (tx, rx) = channel::oneshot();
thread::spawn(move || {
    let result = command.status().map_err(QantoAsyncError::from);
    tx.send(result).ok();
});
rx.recv().await??
        }
    }
}

/// Global runtime instance
static RUNTIME: std::sync::OnceLock<QantoExecutor> = std::sync::OnceLock::new();

/// Initialize the global runtime
pub fn init_runtime(thread_count: usize) {
    RUNTIME.set(QantoExecutor::new(thread_count)).unwrap();
}

/// Get the global runtime
pub fn runtime() -> &'static QantoExecutor {
    RUNTIME.get().expect("Runtime not initialized. Call init_runtime() first.")
}

/// Spawn a task on the global runtime
pub fn spawn<F>(future: F) -> TaskId
where
    F: Future<Output = ()> + Send + 'static,
{
    runtime().spawn(future)
}

/// Block on a future using the global runtime
pub fn block_on<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    runtime().block_on(future)
}

/// Macro for main function
#[macro_export]
macro_rules! qanto_main {
    ($($item:item)*) => {
        $($item)*
        
        fn main() {
            $crate::qanto_async::init_runtime(num_cpus::get());
            $crate::qanto_async::block_on(async_main());
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_executor_creation() {
        let executor = QantoExecutor::new(2);
        assert_eq!(executor.thread_count, 2);
        executor.shutdown();
    }
    
    #[test]
    fn test_timer() {
        init_runtime(1);
        
        let start = Instant::now();
        block_on(async {
            sleep(Duration::from_millis(100)).await;
        });
        let elapsed = start.elapsed();
        
        assert!(elapsed >= Duration::from_millis(90));
        assert!(elapsed <= Duration::from_millis(200));
    }
    
    #[test]
    fn test_channel() {
        init_runtime(1);
        
        block_on(async {
            let (tx, mut rx) = channel::unbounded();
            
            tx.send(42).unwrap();
            let received = rx.recv().await.unwrap();
            assert_eq!(received, 42);
        });
    }
    
    #[test]
    fn test_oneshot_channel() {
        init_runtime(1);
        
        block_on(async {
            let (tx, rx) = channel::oneshot();
            
            tx.send("hello").unwrap();
            let received = rx.recv().await.unwrap();
            assert_eq!(received, "hello");
        });
    }
}