export type AppInfo = {
  name: string;
  version: string;
};

export enum TaskStatus {
  Idle = "idle",
  Running = "running",
  Done = "done",
  Error = "error",
}

export type PublishState = "unpublished" | "published";

export type ViewMode = "ticket" | "day";

export type ModalKind = "start" | "edit" | "publish";

export type ConnectionType = "jira-cloud" | "jira-dc";

export type ConnectionSetupStep = "details" | "credentials";

export type GuiConnection = {
  id: number;
  name: string;
  type: ConnectionType;
  hostname: string;
  username: string;
};

export type CreateConnectionInput = {
  name: string;
  type: ConnectionType;
  hostname: string;
  username: string;
  token: string;
};

export type BackendSlot = {
  id: number;
  ticket_key: string;
  summary: string | null;
  note: string | null;
  started_at: number;
  stopped_at: number | null;
  published_at: number | null;
  issue_url: string | null;
};

export type BackendTicket = {
  id: number;
  ticket_key: string;
  summary: string | null;
  issue_url: string | null;
};

export type BackendPublishFailure = {
  ticket_key: string;
  error: string;
};

export type BackendPublishResult = {
  published_count: number;
  failed: BackendPublishFailure[];
  timestamp: number;
};

export type TaskSession = {
  id: number;
  taskId: number;
  start: Date;
  end: Date | null;
  note?: string;
  publishState: PublishState;
};

export type Task = {
  id: number;
  key: string;
  title: string;
  status: TaskStatus;
  url?: string;
  sessions: TaskSession[];
};

export type TaskSummary = {
  task: Task;
  sessions: TaskSession[];
  slotCount: number;
  totalMs: number;
  unpublishedMs: number;
  isActive: boolean;
};

export type TaskTimelineSession = {
  task: Task;
  session: TaskSession;
  durationMs: number;
  isActive: boolean;
};

export type TaskSessionEntry = {
  task: Task;
  session: TaskSession;
};

export type StartTaskInput = {
  ticketKey: string;
  title?: string;
  note?: string;
  start?: Date;
};

export type EditSessionInput = {
  sessionId: number;
  ticketKey: string;
  title?: string;
  start: Date;
  end: Date | null;
  note?: string;
};
