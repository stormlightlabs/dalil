+++
id = "arcl-t-01M65H08VQCVS83T5EDQVJFJ7B"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Move standard limitations out of human output"
status = "pending"
priority = "normal"
position = 3
+++

Every human report ends with 5 to 7 lines of standard limitation text. It
costs output budget on every call and repeats across commands.

Acceptance criteria:

- [ ] Default human output omits limitations that apply to every run
- [ ] `--json` and `dalil explain` still carry them in full
- [ ] Limitations specific to one result, such as a reached limit, still
      appear in human output
