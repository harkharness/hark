/** Mirror of domain::task_nudge: a chat with fewer turns of yours than
 *  this is small talk still. */
export const MIN_USER_TURNS = 3;

/** Whether the line "este chat não tem task · registrar?" shows. */
export const offerTask = (userTurns: number, hasTask: boolean, dismissed: boolean) =>
  !hasTask && !dismissed && userTurns >= MIN_USER_TURNS;
