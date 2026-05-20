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

export type ModalKind = "start" | "edit";

export type ConnectionType = "jira-cloud" | "jira-dc";

export type ConnectionSetupStep = "details" | "credentials";

export type GuiConnection = {
  type: ConnectionType;
  hostname: string;
  username: string;
  token: string;
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
};

export type EditSessionInput = {
  sessionId: number;
  ticketKey: string;
  title?: string;
  start: Date;
  end: Date | null;
  note?: string;
};
