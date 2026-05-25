import {
  Task,
  TaskSession,
  TaskSessionEntry,
  TaskStatus,
  TaskSummary,
  TaskTimelineSession,
} from "../types/types";
import {
  dayBounds,
  sessionEndsAfterDay,
  sessionOverlapMs,
  sessionOverlapsDay,
  sessionStartsBeforeDay,
} from "./time";

export const activeTaskFor = (tasks: Task[]): Task | null =>
  tasks.find((task) => task.status === TaskStatus.Running) ?? null;

export const activeSessionFor = (tasks: Task[]): TaskSession | null =>
  tasks
    .flatMap((task) => task.sessions)
    .find((session) => session.end === null) ?? null;

export const previousTaskFor = (
  tasks: Task[],
  activeTaskId: number | null,
): Task | null => {
  if (activeTaskId === null) {
    return null;
  }

  const previousEntry = tasks
    .flatMap((task) =>
      task.sessions.map((session) => ({
        task,
        startedAt: session.start.getTime(),
      })),
    )
    .filter((entry) => entry.task.id !== activeTaskId)
    .sort((left, right) => right.startedAt - left.startedAt)[0];

  return previousEntry?.task ?? null;
};

export const selectedTaskFor = (
  tasks: Task[],
  selectedTaskId: number | null,
): Task | null => tasks.find((task) => task.id === selectedTaskId) ?? null;

export const selectedSessionEntryFor = (
  tasks: Task[],
  selectedSessionId: number | null,
): TaskSessionEntry | null => {
  for (const task of tasks) {
    const session = task.sessions.find(
      (candidate) => candidate.id === selectedSessionId,
    );

    if (session) {
      return { task, session };
    }
  }

  return null;
};

export const summariesForDay = (
  tasks: Task[],
  selectedDate: Date,
  now: Date,
): TaskSummary[] =>
  tasks
    .map((task) => {
      const sessions = task.sessions.filter((session) =>
        sessionOverlapsDay(session, selectedDate, now),
      );
      const totalMs = sessions.reduce(
        (sum, session) => sum + sessionOverlapMs(session, selectedDate, now),
        0,
      );
      const unpublishedMs = sessions
        .filter((session) => session.publishState === "unpublished")
        .reduce(
          (sum, session) => sum + sessionOverlapMs(session, selectedDate, now),
          0,
        );

      return {
        task,
        sessions,
        slotCount: sessions.length,
        totalMs,
        unpublishedMs,
        isActive: task.status === TaskStatus.Running,
      };
    })
    .filter((summary) => summary.slotCount > 0);

export const timelineSessionsForDay = (
  tasks: Task[],
  selectedDate: Date,
  now: Date,
): TaskTimelineSession[] =>
  tasks
    .flatMap((task) =>
      task.sessions
        .filter((session) => sessionOverlapsDay(session, selectedDate, now))
        .map((session) => {
          const startsBeforeDay = sessionStartsBeforeDay(session, selectedDate);
          const endsAfterDay = sessionEndsAfterDay(session, selectedDate, now);

          return {
            task,
            session,
            durationMs: sessionOverlapMs(session, selectedDate, now),
            startsBeforeDay,
            endsAfterDay,
            isMultiDay: startsBeforeDay || endsAfterDay,
            isActive: session.end === null,
          };
        }),
    )
    .sort(
      (left, right) => {
        const bounds = dayBounds(selectedDate);
        const leftStart = Math.max(
          left.session.start.getTime(),
          bounds.start.getTime(),
        );
        const rightStart = Math.max(
          right.session.start.getTime(),
          bounds.start.getTime(),
        );

        return leftStart - rightStart;
      },
    );

export const totalDurationForDay = (
  tasks: Task[],
  selectedDate: Date,
  now: Date,
) =>
  tasks
    .flatMap((task) => task.sessions)
    .filter((session) => sessionOverlapsDay(session, selectedDate, now))
    .reduce(
      (sum, session) => sum + sessionOverlapMs(session, selectedDate, now),
      0,
    );

export const unpublishedDurationForDay = (
  tasks: Task[],
  selectedDate: Date,
  now: Date,
) =>
  tasks
    .flatMap((task) => task.sessions)
    .filter(
      (session) =>
        session.publishState === "unpublished" &&
        sessionOverlapsDay(session, selectedDate, now),
    )
    .reduce(
      (sum, session) => sum + sessionOverlapMs(session, selectedDate, now),
      0,
    );

export const hasMultiDaySessionsForDay = (
  tasks: Task[],
  selectedDate: Date,
  now: Date,
) =>
  tasks
    .flatMap((task) => task.sessions)
    .some(
      (session) =>
        sessionOverlapsDay(session, selectedDate, now) &&
        (sessionStartsBeforeDay(session, selectedDate) ||
          sessionEndsAfterDay(session, selectedDate, now)),
    );
