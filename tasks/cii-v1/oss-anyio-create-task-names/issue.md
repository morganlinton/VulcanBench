# create_task default task names omit the module name

`TaskGroup.start_soon(func)` names the spawned task with the function's
module-qualified name (`mypkg.worker.taskfunc`). The analogous
`TaskGroup.create_task(func())` does not — its default name is just the
coroutine's qualname, with no module:

```python
async with anyio.create_task_group() as tg:
    tg.start_soon(taskfunc)          # task name: "mypkg.worker.taskfunc"
    h = tg.create_task(taskfunc())   # h.name:    "taskfunc"
```

The two spellings spawn the same work; debugging output and task
introspection should name them the same way.

Expected: the default name for a `create_task` task includes the coroutine's
module (module-qualified, joined with a dot), matching `start_soon`'s
convention, on **both** the asyncio and trio backends. When the module can't
be determined, fall back to the qualname alone. An explicit `name=` argument
still wins verbatim, `start_soon` naming is unchanged, and awaiting the
returned handle still yields the task's result.
