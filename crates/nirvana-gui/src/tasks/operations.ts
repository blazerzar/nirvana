import { ticketUrlForKey } from "../tickets";
import { EditSessionInput, Task, TaskSession, TaskStatus } from "../types/types";
import { selectedSessionEntryFor } from "./selectors";
import { isSameDay } from "./time";

export const normalizeTicketKey = (ticketKey: string) =>
  ticketKey.trim().toUpperCase();

export const normalizedNote = (note?: string) => {
  const trimmed = note?.trim();
  return trimmed ? trimmed : undefined;
};

export const nextTaskId = (tasks: Task[]) =>
  Math.max(0, ...tasks.map((task) => task.id)) + 1;

export const nextSessionId = (tasks: Task[]) =>
  Math.max(
    0,
    ...tasks.flatMap((item) => item.sessions.map((session) => session.id)),
  ) + 1;

export const findTaskByKey = (tasks: Task[], ticketKey: string) => {
  const key = normalizeTicketKey(ticketKey);
  return tasks.find((task) => task.key.toUpperCase() === key) ?? null;
};

export const ensureTaskForKey = (
  tasks: Task[],
  ticketKey: string,
  title?: string,
) => {
  const key = normalizeTicketKey(ticketKey);
  const existingTask = findTaskByKey(tasks, key);

  if (existingTask) {
    return existingTask;
  }

  const task: Task = {
    id: nextTaskId(tasks),
    key,
    title: title?.trim() || key,
    status: TaskStatus.Idle,
    lastWorkedAt: 0,
    url: ticketUrlForKey(key),
    sessions: [],
  };

  tasks.push(task);
  return task;
};

export const stopActiveTask = (tasks: Task[], now: Date) => {
  const activeTask = tasks.find((task) => task.status === TaskStatus.Running);
  const activeSession =
    tasks
      .flatMap((task) => task.sessions)
      .find((session) => session.end === null) ?? null;

  if (!activeTask || !activeSession) {
    return false;
  }

  activeSession.end = new Date(now);
  activeTask.status = TaskStatus.Idle;
  return true;
};

export const startTaskSession = (
  tasks: Task[],
  taskId: number,
  now: Date,
  note?: string,
) => {
  stopActiveTask(tasks, now);

  const task = tasks.find((candidate) => candidate.id === taskId);
  if (!task) {
    return null;
  }

  const session: TaskSession = {
    id: nextSessionId(tasks),
    taskId: task.id,
    start: new Date(now),
    end: null,
    publishState: "unpublished",
    note: normalizedNote(note),
  };

  task.status = TaskStatus.Running;
  task.lastWorkedAt = Math.max(task.lastWorkedAt, session.start.getTime());
  task.sessions.push(session);
  return { task, session };
};

export const updateSession = (tasks: Task[], input: EditSessionInput) => {
  const currentTask = tasks.find((task) =>
    task.sessions.some((session) => session.id === input.sessionId),
  );
  const session = currentTask?.sessions.find(
    (candidate) => candidate.id === input.sessionId,
  );

  if (!currentTask || !session || session.publishState === "published") {
    return null;
  }

  const targetTask = ensureTaskForKey(tasks, input.ticketKey, input.title);
  session.start = new Date(input.start);
  session.end = input.end ? new Date(input.end) : null;
  session.note = normalizedNote(input.note);
  targetTask.lastWorkedAt = Math.max(
    targetTask.lastWorkedAt,
    session.start.getTime(),
  );

  if (targetTask.id !== currentTask.id) {
    currentTask.sessions = currentTask.sessions.filter(
      (candidate) => candidate.id !== session.id,
    );
    session.taskId = targetTask.id;
    targetTask.sessions.push(session);
  }

  currentTask.status = currentTask.sessions.some((candidate) => candidate.end === null)
    ? TaskStatus.Running
    : TaskStatus.Idle;
  targetTask.status = targetTask.sessions.some((candidate) => candidate.end === null)
    ? TaskStatus.Running
    : targetTask.status === TaskStatus.Running
      ? TaskStatus.Idle
      : targetTask.status;

  return { task: targetTask, session };
};

export const deleteSession = (
  tasks: Task[],
  selectedSessionId: number | null,
) => {
  const entry = selectedSessionEntryFor(tasks, selectedSessionId);

  if (!entry || entry.session.publishState === "published") {
    return null;
  }

  entry.task.sessions = entry.task.sessions.filter(
    (session) => session.id !== entry.session.id,
  );
  entry.task.status = entry.task.sessions.some((session) => session.end === null)
    ? TaskStatus.Running
    : TaskStatus.Idle;
  return entry.task;
};

export const publishUnpublishedForDay = (tasks: Task[], selectedDate: Date) => {
  tasks.forEach((task) => {
    task.sessions.forEach((session) => {
      if (
        session.publishState === "unpublished" &&
        isSameDay(session.start, selectedDate)
      ) {
        session.publishState = "published";
      }
    });
  });
};
