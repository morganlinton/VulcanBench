from orderflow.ofkit.runner import main
from orderflow.billing.app import router

main(router, migrations="db/billing", database="billing_db")
