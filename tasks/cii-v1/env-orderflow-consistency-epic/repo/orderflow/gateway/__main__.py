from orderflow.ofkit.runner import main
from orderflow.gateway.app import router

main(router, migrations=None, database=None)
