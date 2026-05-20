import { Task, TaskStatus } from "./types/types";
import { ticketUrlForKey } from "./tickets";

const dateAt = (dayOffset: number, hour: number, minute = 0) => {
  const date = new Date();
  date.setHours(hour, minute, 0, 0);
  date.setDate(date.getDate() + dayOffset);
  return date;
};

const minutesAgo = (minutes: number) =>
  new Date(Date.now() - minutes * 60 * 1000);

const task = (item: Omit<Task, "url">): Task => ({
  ...item,
  url: ticketUrlForKey(item.key),
});

export const createTasksData = (): Task[] => [
  task({
    id: 12,
    key: "NIR-1",
    title: "Shape the tracker foundation",
    status: TaskStatus.Running,
    sessions: [
      {
        id: 142,
        taskId: 12,
        start: dateAt(0, 9, 15),
        end: dateAt(0, 9, 48),
        note: "rebased onto main",
        publishState: "published",
      },
      {
        id: 145,
        taskId: 12,
        start: dateAt(0, 11, 2),
        end: dateAt(0, 11, 50),
        note: "paired with Anna",
        publishState: "unpublished",
      },
      {
        id: 148,
        taskId: 12,
        start: dateAt(0, 13, 30),
        end: dateAt(0, 14, 16),
        publishState: "unpublished",
      },
      {
        id: 153,
        taskId: 12,
        start: minutesAgo(23),
        end: null,
        note: "polish pass",
        publishState: "unpublished",
      },
    ],
  }),
  task({
    id: 9,
    key: "NIR-9",
    title: "Refine command bar",
    status: TaskStatus.Idle,
    sessions: [
      {
        id: 127,
        taskId: 9,
        start: dateAt(0, 8, 5),
        end: dateAt(0, 8, 30),
        publishState: "published",
      },
    ],
  }),
  task({
    id: 241,
    key: "OPS-241",
    title: "Workspace connection review",
    status: TaskStatus.Idle,
    sessions: [
      {
        id: 131,
        taskId: 241,
        start: dateAt(0, 10, 5),
        end: dateAt(0, 10, 29),
        note: "credential check",
        publishState: "published",
      },
      {
        id: 132,
        taskId: 241,
        start: dateAt(0, 15, 8),
        end: dateAt(0, 15, 36),
        publishState: "unpublished",
      },
    ],
  }),
  task({
    id: 3,
    key: "NIR-3",
    title: "Design empty and loading states",
    status: TaskStatus.Done,
    sessions: [
      {
        id: 118,
        taskId: 3,
        start: dateAt(0, 16, 4),
        end: dateAt(0, 16, 8),
        publishState: "published",
      },
    ],
  }),
  task({
    id: 58,
    key: "DSGN-58",
    title: "Motion language audit",
    status: TaskStatus.Idle,
    sessions: [
      {
        id: 109,
        taskId: 58,
        start: dateAt(-1, 14, 0),
        end: dateAt(-1, 14, 42),
        publishState: "published",
      },
      {
        id: 110,
        taskId: 58,
        start: dateAt(-1, 16, 20),
        end: dateAt(-1, 16, 50),
        publishState: "published",
      },
    ],
  }),
  task({
    id: 77,
    key: "INF-77",
    title: "Tauri app metadata fallback",
    status: TaskStatus.Idle,
    sessions: [
      {
        id: 101,
        taskId: 77,
        start: dateAt(-2, 12, 10),
        end: dateAt(-2, 12, 31),
        publishState: "unpublished",
      },
    ],
  }),
  task({
    id: 14,
    key: "NIR-14",
    title: "Keyboard shortcut pass",
    status: TaskStatus.Idle,
    sessions: [
      {
        id: 99,
        taskId: 14,
        start: dateAt(-4, 9, 0),
        end: dateAt(-4, 9, 15),
        publishState: "published",
      },
    ],
  }),
];
