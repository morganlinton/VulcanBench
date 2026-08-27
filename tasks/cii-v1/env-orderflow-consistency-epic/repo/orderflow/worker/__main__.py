from orderflow.ofkit.runner import main
from orderflow.worker.app import router, start_autopoll

start_autopoll()
main(router, migrations=None, database=None)
