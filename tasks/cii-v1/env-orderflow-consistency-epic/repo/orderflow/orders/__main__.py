from orderflow.ofkit.runner import main
from orderflow.orders.app import router

main(router, migrations="db/orders", database="orders_db")
