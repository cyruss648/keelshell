import importlib.util, pathlib, sys, unittest
sys.dont_write_bytecode=True
root=pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0,str(root/'scripts'))
import run_linux_local_backend as current
import test_linux_local_backend as cases
spec=importlib.util.spec_from_file_location('before_fix',pathlib.Path(__file__).resolve().parent/'native-r2/host-runner-inputs/run_linux_local_backend.py')
before=importlib.util.module_from_spec(spec)
spec.loader.exec_module(before)
before.ROOT=current.ROOT
cases.backend=before
result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(cases.TopLevelFailureContracts))
sys.exit(0 if result.wasSuccessful() else 1)
