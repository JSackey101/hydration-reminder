# State machines

## Reminder lifecycle

```mermaid
stateDiagram-v2
    [*] --> Waiting
    Waiting --> Prompting: now >= next_due_at
    Waiting --> Waiting: tray "Log a drink" / count+1, next_due = now+interval
    Waiting --> Waiting: tray "Undo last drink" / count-1, next_due = previous value
    Prompting --> Waiting: Drank / count+1, next_due = now+interval
    Prompting --> Waiting: tray "Log a drink" / count+1, close popup, next_due = now+interval
    Prompting --> Waiting: timeout (if set) / close popup, next_due = now+interval
    Prompting --> Snoozed: Snooze / next_due = now+10 min
    Snoozed --> Prompting: now >= next_due_at
    Snoozed --> Waiting: tray "Log a drink" / count+1, next_due = now+interval
```

- **Wake from sleep:** if any due time passed while asleep, go to a single Prompting; missed reminders don't stack.
- **Day rollover:** on any tick or startup where the local date has changed, count → 0 and the undo history clears. The lifecycle state is unaffected.
- **Prompting** holds until answered, unless the optional timeout is set.
- **Undo** after a drink lands back in Waiting with the restored due time; if that time has already passed, the next tick prompts.
