"""Build a VulcanBench suite from merged pull requests in your own repository.

The flow, one ``vulcanbench suite`` subcommand per step (all take ``--json``):

    suite unlock                                   # the builder is password-gated
    suite init <id> --display-name NAME --repo owner/name
    suite prs list --repo owner/name               # merged PRs with a suitability score
    suite task scaffold <id> --pr N                # task dir at the PR's base commit
    suite task author --task-dir T --harness H --model M   # one model session (quota)
    suite validate <id> --task <task>              # gold solves, base fails, no answer leak
    suite add-task <id> --task <task>
    suite freeze <id>

``access`` holds the password gate, ``gh`` the GitHub CLI wrapper, ``prs`` the
pull-request listing, ``scaffold`` the task scaffold, ``prompt`` and
``author`` the authoring session, ``commands`` the CLI wiring.
"""
