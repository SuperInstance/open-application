//! Agent Runtime — Minimal agent runtime that runs inside the Tauri application.
//!
//! The agent runtime drives the UI by maintaining an internal state machine with
//! conservation budget, task queue, and spectral feedback loop. The agent IS
//! the application — it doesn't generate UI code, it directly controls what
//! the UI shows through its state transitions.
//!
//! # Conservation Budget
//!
//! The agent operates under a conservation budget that limits total activity.
//! Each action consumes budget; completed actions release it. The spectral
//! feedback loop adjusts the budget allocation based on eigenvalue analysis
//! of recent performance.

/// Actions the agent can take during a tick.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    /// Process the next task in the queue.
    ProcessTask { task_id: u64, name: String },
    /// Update the UI with new data.
    UpdateUI { component: String, payload: String },
    /// The agent is idle — no tasks to process.
    Idle,
    /// Request more budget from the system.
    RequestBudget { requested: f64 },
    /// Report a spectral analysis result.
    SpectralFeedback { eigenvalue: f64, bottleneck: Option<u64> },
}

/// A task in the agent's queue.
#[derive(Debug, Clone)]
pub struct AgentTask {
    pub id: u64,
    pub name: String,
    pub priority: f64,
    pub payload: String,
}

/// The state of the agent runtime.
#[derive(Debug, Clone)]
pub struct AgentState {
    /// Conservation budget — total energy available.
    pub budget: f64,
    /// Current budget consumed.
    pub consumed: f64,
    /// Task queue ordered by priority.
    pub task_queue: Vec<AgentTask>,
    /// Spectral feedback history (recent eigenvalues).
    pub spectral_history: Vec<f64>,
    /// Number of completed tasks.
    pub completed_count: u64,
    /// Number of ticks elapsed.
    pub tick_count: u64,
    /// Next task ID.
    next_task_id: u64,
}

/// The agent runtime that runs inside the Tauri app.
pub struct AgentRuntime {
    /// Current agent state.
    pub state: AgentState,
    /// Maximum budget the agent can hold.
    max_budget: f64,
    /// Number of spectral history entries to retain.
    spectral_window: usize,
}

impl AgentRuntime {
    /// Create a new agent runtime with the given conservation budget.
    pub fn new(budget: f64) -> Self {
        Self {
            state: AgentState {
                budget,
                consumed: 0.0,
                task_queue: Vec::new(),
                spectral_history: Vec::new(),
                completed_count: 0,
                tick_count: 0,
                next_task_id: 0,
            },
            max_budget: budget,
            spectral_window: 10,
        }
    }

    /// Set the spectral feedback window size.
    pub fn with_spectral_window(mut self, window: usize) -> Self {
        self.spectral_window = window;
        self
    }

    /// Enqueue a task for the agent to process.
    ///
    /// Returns the task ID. Tasks are inserted in priority order (highest first).
    pub fn enqueue(&mut self, name: &str, priority: f64, payload: &str) -> u64 {
        let id = self.state.next_task_id;
        self.state.next_task_id += 1;

        let task = AgentTask {
            id,
            name: name.to_string(),
            priority,
            payload: payload.to_string(),
        };

        // Insert in priority order
        let pos = self
            .state
            .task_queue
            .iter()
            .position(|t| t.priority < priority)
            .unwrap_or(self.state.task_queue.len());

        self.state.task_queue.insert(pos, task);
        id
    }

    /// Perform one tick of the agent runtime.
    ///
    /// Processes the highest-priority task if budget allows, runs spectral
    /// feedback analysis, and returns the action taken.
    pub fn tick(&mut self) -> AgentAction {
        self.state.tick_count += 1;

        // Check if we have budget and tasks
        let remaining = self.state.budget - self.state.consumed;
        if remaining <= 0.0 && !self.state.task_queue.is_empty() {
            return AgentAction::RequestBudget {
                requested: self.state.task_queue[0].priority.min(1.0),
            };
        }

        if self.state.task_queue.is_empty() {
            return AgentAction::Idle;
        }

        // Process highest-priority task
        let task = self.state.task_queue.remove(0);
        let cost = task.priority.min(remaining);

        // Check budget
        if cost <= 0.0 {
            // Can't afford this task — put it back
            let priority = task.priority;
            self.state.task_queue.insert(0, task);
            return AgentAction::RequestBudget {
                requested: priority,
            };
        }

        self.state.consumed += cost;
        self.state.completed_count += 1;

        // Release budget (conservation: completed work returns energy)
        let release = cost * 0.8; // 80% conservation efficiency
        self.state.consumed -= release;

        // Update spectral history
        let eigenvalue = self.compute_spectral_feedback();
        self.state.spectral_history.push(eigenvalue);
        if self.state.spectral_history.len() > self.spectral_window {
            self.state.spectral_history.remove(0);
        }

        let _bottleneck = self.detect_bottleneck();

        AgentAction::ProcessTask {
            task_id: task.id,
            name: task.name,
        }
    }

    /// Compute spectral feedback from recent task processing.
    ///
    /// Simulates eigenvalue computation on the task priority distribution.
    /// In production, this would call the actual spectral scheduler.
    fn compute_spectral_feedback(&self) -> f64 {
        if self.state.task_queue.is_empty() {
            return 0.0;
        }

        // Approximate dominant eigenvalue as max priority ratio
        let priorities: Vec<f64> = self
            .state
            .task_queue
            .iter()
            .map(|t| t.priority)
            .collect();

        if priorities.is_empty() {
            return 0.0;
        }

        // Simple power iteration on a 1D projection
        let sum: f64 = priorities.iter().sum();
        let max = priorities.iter().cloned().fold(0.0_f64, f64::max);

        if sum == 0.0 {
            return 0.0;
        }

        // Spectral ratio: how concentrated is the priority mass
        max / sum
    }

    /// Detect the current bottleneck based on spectral feedback.
    ///
    /// Returns the task ID of the highest-priority task if the spectral
    /// concentration is above a threshold (indicating a bottleneck).
    fn detect_bottleneck(&self) -> Option<u64> {
        if self.state.spectral_history.is_empty() {
            return None;
        }

        let recent_ev = self.state.spectral_history.last()?;
        let threshold = 0.5;

        if *recent_ev > threshold {
            self.state.task_queue.first().map(|t| t.id)
        } else {
            None
        }
    }

    /// Get the current budget utilization as a fraction [0, 1].
    pub fn budget_utilization(&self) -> f64 {
        if self.state.budget == 0.0 {
            return 0.0;
        }
        self.state.consumed / self.state.budget
    }

    /// Get the number of pending tasks.
    pub fn pending_tasks(&self) -> usize {
        self.state.task_queue.len()
    }

    /// Reset consumed budget (e.g., after a full cycle).
    pub fn reset_budget(&mut self) {
        self.state.consumed = 0.0;
    }

    /// Get the average eigenvalue from the spectral history.
    pub fn average_spectral_energy(&self) -> f64 {
        if self.state.spectral_history.is_empty() {
            return 0.0;
        }
        self.state.spectral_history.iter().sum::<f64>() / self.state.spectral_history.len() as f64
    }
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_runtime() {
        let rt = AgentRuntime::new(10.0);
        assert_eq!(rt.state.budget, 10.0);
        assert_eq!(rt.state.consumed, 0.0);
        assert!(rt.state.task_queue.is_empty());
    }

    #[test]
    fn test_enqueue_task() {
        let mut rt = AgentRuntime::new(10.0);
        let id = rt.enqueue("task1", 0.5, "payload");
        assert_eq!(id, 0);
        assert_eq!(rt.pending_tasks(), 1);
    }

    #[test]
    fn test_enqueue_priority_ordering() {
        let mut rt = AgentRuntime::new(10.0);
        rt.enqueue("low", 0.1, "");
        rt.enqueue("high", 0.9, "");
        rt.enqueue("mid", 0.5, "");

        assert_eq!(rt.state.task_queue[0].name, "high");
        assert_eq!(rt.state.task_queue[1].name, "mid");
        assert_eq!(rt.state.task_queue[2].name, "low");
    }

    #[test]
    fn test_tick_processes_task() {
        let mut rt = AgentRuntime::new(10.0);
        rt.enqueue("task1", 0.5, "data");

        let action = rt.tick();
        match action {
            AgentAction::ProcessTask { name, .. } => assert_eq!(name, "task1"),
            _ => panic!("Expected ProcessTask, got {:?}", action),
        }
        assert_eq!(rt.state.completed_count, 1);
        assert_eq!(rt.pending_tasks(), 0);
    }

    #[test]
    fn test_tick_idle_when_empty() {
        let mut rt = AgentRuntime::new(10.0);
        let action = rt.tick();
        assert_eq!(action, AgentAction::Idle);
    }

    #[test]
    fn test_tick_requests_budget_when_exhausted() {
        let mut rt = AgentRuntime::new(0.0);
        rt.enqueue("task1", 0.5, "");

        let action = rt.tick();
        assert!(matches!(action, AgentAction::RequestBudget { .. }));
    }

    #[test]
    fn test_budget_utilization() {
        let mut rt = AgentRuntime::new(10.0);
        assert_eq!(rt.budget_utilization(), 0.0);

        rt.enqueue("task1", 0.5, "");
        rt.tick();

        // After processing, some budget was consumed and partially released
        let util = rt.budget_utilization();
        assert!(util >= 0.0 && util <= 1.0);
    }

    #[test]
    fn test_spectral_feedback_updates() {
        let mut rt = AgentRuntime::new(10.0);
        rt.enqueue("task1", 0.8, "");
        rt.tick();

        assert!(!rt.state.spectral_history.is_empty());
        let ev = rt.state.spectral_history[0];
        assert!(ev >= 0.0 && ev <= 1.0);
    }

    #[test]
    fn test_spectral_window_limit() {
        let mut rt = AgentRuntime::new(100.0).with_spectral_window(3);
        for i in 0..5 {
            rt.enqueue(&format!("task{}", i), 0.5, "");
            rt.tick();
        }
        assert!(rt.state.spectral_history.len() <= 3);
    }

    #[test]
    fn test_average_spectral_energy() {
        let mut rt = AgentRuntime::new(10.0);
        // Enqueue multiple tasks so queue isn't empty after first tick
        rt.enqueue("t1", 0.5, "");
        rt.enqueue("t2", 0.8, "");
        rt.tick();
        rt.tick();

        let avg = rt.average_spectral_energy();
        // After processing both, spectral history should have entries
        // At least the first tick should have had a non-zero eigenvalue
        assert!(!rt.state.spectral_history.is_empty());
    }

    #[test]
    fn test_reset_budget() {
        let mut rt = AgentRuntime::new(10.0);
        rt.enqueue("task1", 0.5, "");
        rt.tick();
        assert!(rt.state.consumed > 0.0);
        rt.reset_budget();
        assert_eq!(rt.state.consumed, 0.0);
    }

    #[test]
    fn test_tick_counter_increments() {
        let mut rt = AgentRuntime::new(10.0);
        assert_eq!(rt.state.tick_count, 0);
        rt.tick();
        assert_eq!(rt.state.tick_count, 1);
        rt.tick();
        assert_eq!(rt.state.tick_count, 2);
    }
}
