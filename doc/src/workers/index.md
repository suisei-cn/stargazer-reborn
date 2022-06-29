# Workers

In stargazer-reborn, sources to be traced are called *tasks*.
A task can be a Twitter account, a YouTube channel, a Twitch channel, etc.
All tasks are stored in the `tasks` collection in MongoDB.

Each task is handled by a worker.
Say if there's a task of Twitter account `@suisei_hosimati`, the worker is responsible for fetching the latest tweets of this account, and pushing them to the message queue. And the relation between tasks and workers is one-to-many.

To handle a great amount of tasks, stargazer-reborn uses a worker cluster.
The cluster is heterogeneous in the sense that each worker can handle only one kind of task. However, every worker is *equivalent* if they are of the same kind, i.e. there's no "central" or "master" worker, nor any kind of "coordinator" node.
Each worker connects to the database, the single source of truth, and the message queue, on its own.

To evenly distribute the work load, each worker uses gossip-based SWIM protocol to discover the other workers in the cluster. After that, a consistent hash ring is built to determine the worker responsible for a given task.