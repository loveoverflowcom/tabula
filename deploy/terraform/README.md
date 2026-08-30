# terraform/

**Deliberately empty. Stage 2+, and conditional.**

Doc 09 §2 lists the deployment platform as replaceable behind "one binary +
Postgres, no platform coupling", at a cost of "a Terraform/systemd change".
That property only holds while nothing here encodes a topology.

Add contents when — and only when — doc 06 §1.1's Stage 2 trigger has fired:
more than one match-owning process actually exists.

Until then: `deploy/systemd/` and a VPS.
