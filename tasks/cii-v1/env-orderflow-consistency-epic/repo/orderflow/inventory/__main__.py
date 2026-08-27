from orderflow.ofkit.runner import main
from orderflow.inventory.app import router

main(router, migrations="db/inventory", database="inventory_db")
