# Preset projects

Small repositories used by `features/presets.feature` and `features/without.feature`. The step `Given the project "<name>"` copies one into the scenario's empty repository and replaces `@PACKAGE@` in every `.pkl` file with the path of the package under test. A project is a `pantheon.pkl` and a `presets/` directory, as a real repository has them.
