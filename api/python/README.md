<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->

# Slint-python (Alpha)

[Slint](https://slint.dev/) is a UI toolkit that supports different programming languages.
Slint-python is the integration with Python.

**Warning: Alpha**
Slint-python is still in the very early stages of development: APIs will change and important features are still being developed,
the project is overall incomplete.

You can track the overall progress for the Python integration in GitHub at https://github.com/slint-ui/slint/milestone/18
as well as by looking at python-labelled issues at https://github.com/slint-ui/slint/labels/a%3Alanguage-python .

If you want to just play with this, you can try running one of our test cases in a virtual environment:

```bash
cd api/python
python -m env .env
source .env/bin/activate
pip install maturin
maturin develop
python ./tests/test_instance.py
```

This will bring up the printer demo and a Python callback is invoked when starting a new print job.

