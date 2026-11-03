+++
id = "arcl-t-01M65H08VQCVS83T5EDQVJFJ7B"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Move standard limitations out of human output"
status = "completed"
priority = "normal"
position = 3
evidence = "Done in #6 at 6803e61: caveats that hold for every run live in dalil_core::limitations; human Markdown and HTML print one note in their place, JSON keeps them, and explain lists them inside the budget."
+++

Every human report ends with 5 to 7 lines of standard limitation text. It
costs output budget on every call and repeats across commands.

Acceptance criteria:

- [ ] Default human output omits limitations that apply to every run
- [ ] `--json` and `dalil explain` still carry them in full
- [ ] Limitations specific to one result, such as a reached limit, still
      appear in human output
