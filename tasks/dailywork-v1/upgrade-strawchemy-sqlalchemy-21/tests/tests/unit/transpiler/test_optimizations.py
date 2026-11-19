"""DB-free unit tests for transpiler SQL optimizations."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest
from inline_snapshot import snapshot

from tests.unit.schemas.optimizations import schema
from tests.unit.utils import SQLA_DIALECTS, DialectContext
from tests.utils import format_sql

if TYPE_CHECKING:
    from sqlalchemy import Select

# Module-level snapshots storing every (query, dialect) combination; required because a single
# ``snapshot()`` literal cannot be a param under stacked parametrize (cross-product).

AGGREGATION_SQL = snapshot(
    {
        "output-order-by-postgresql": [
            "SELECT color.id,",
            "       anon_1.count_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY anon_1.count_1 ASC",
        ],
        "output-order-by-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, ?) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY coalesce(anon_1.count_1, ?) ASC",
        ],
        "output-order-by-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, %s) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY coalesce(anon_1.count_1, %s) ASC",
        ],
        "output-filter-postgresql": [
            "SELECT color.id,",
            "       anon_1.count_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " WHERE anon_1.count_1 > %(param_1)s",
            " ORDER BY color.id ASC",
        ],
        "output-filter-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, ?) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, ?) > ?",
            " ORDER BY color.id ASC",
        ],
        "output-filter-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, %s) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, %s) > %s",
            " ORDER BY color.id ASC",
        ],
        "filter-order-by-postgresql": [
            "SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1,",
            "               avg(fruit_2.sweetness) AS avg_1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE anon_1.count_1 > %(param_1)s",
            " ORDER BY anon_1.avg_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "filter-order-by-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               avg(fruit_2.sweetness) AS avg_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE coalesce(anon_1.count_1, ?) > ?",
            " ORDER BY anon_1.avg_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "filter-order-by-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               avg(fruit_2.sweetness) AS avg_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE coalesce(anon_1.count_1, %s) > %s",
            " ORDER BY anon_1.avg_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "filter-order-by-same-aggregation-postgresql": [
            "SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE anon_1.avg_1 > %(param_1)s",
            " ORDER BY anon_1.avg_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "filter-order-by-same-aggregation-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE anon_1.avg_1 > ?",
            " ORDER BY anon_1.avg_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "filter-order-by-same-aggregation-mysql": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE anon_1.avg_1 > %s",
            " ORDER BY anon_1.avg_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "output-multiple-aggregations-postgresql": [
            "SELECT color.id,",
            "       anon_1.max_1,",
            "       anon_1.max_2",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT max(fruit_1.sweetness) AS max_1,",
            "               max(fruit_1.name) AS max_2",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC",
        ],
        "output-multiple-aggregations-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT max(fruit_1.sweetness) AS max_1,",
            "               max(fruit_1.name) AS max_2,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       anon_1.max_1,",
            "       anon_1.max_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY color.id ASC",
        ],
        "output-multiple-aggregations-mysql": [
            "WITH anon_1 AS (",
            "        SELECT max(fruit_1.sweetness) AS max_1,",
            "               max(fruit_1.name) AS max_2,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       anon_1.max_1,",
            "       anon_1.max_2",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY color.id ASC",
        ],
        "filter-multiple-aggregations-postgresql": [
            "SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               sum(fruit_2.sweetness) AS sum_1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE anon_1.avg_1 > %(param_1)s",
            "   AND anon_1.sum_1 > %(param_2)s",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "filter-multiple-aggregations-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               sum(fruit_2.sweetness) AS sum_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE anon_1.avg_1 > ?",
            "   AND anon_1.sum_1 > ?",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "filter-multiple-aggregations-mysql": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               sum(fruit_2.sweetness) AS sum_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE anon_1.avg_1 > %s",
            "   AND anon_1.sum_1 > %s",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "order-by-multiple-aggregations-postgresql": [
            "SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               sum(fruit_2.sweetness) AS sum_1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " ORDER BY anon_1.avg_1 ASC,",
            "          anon_1.sum_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "order-by-multiple-aggregations-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               sum(fruit_2.sweetness) AS sum_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " ORDER BY anon_1.avg_1 ASC,",
            "          anon_1.sum_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "order-by-multiple-aggregations-mysql": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_2.sweetness) AS avg_1,",
            "               sum(fruit_2.sweetness) AS sum_1,",
            "               fruit_2.color_id AS color_id",
            "          FROM fruit AS fruit_2",
            "         WHERE fruit_2.color_id IS NOT NULL",
            "         GROUP BY fruit_2.color_id",
            "       ) SELECT color.id,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " ORDER BY anon_1.avg_1 ASC,",
            "          anon_1.sum_1 ASC,",
            "          fruit_1.id ASC",
        ],
        "order-by-and-selected-functions-postgresql": [
            "SELECT color.id,",
            "       anon_1.count_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT sum(fruit_1.sweetness) AS sum_1,",
            "               count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY anon_1.sum_1 ASC",
        ],
        "order-by-and-selected-functions-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT sum(fruit_1.sweetness) AS sum_1,",
            "               count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, ?) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY anon_1.sum_1 ASC",
        ],
        "order-by-and-selected-functions-mysql": [
            "WITH anon_1 AS (",
            "        SELECT sum(fruit_1.sweetness) AS sum_1,",
            "               count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, %s) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY anon_1.sum_1 ASC",
        ],
    }
)

PAGINATION_JOIN_SQL = snapshot(
    {
        "all-functions-hoisted-postgresql": [
            "SELECT color.id",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.count_1 AS count_1",
            "          FROM color AS color",
            "          JOIN LATERAL (",
            "                SELECT count(*) AS count_1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_1",
            "            ON TRUE",
            "         ORDER BY anon_1.count_1 ASC",
            "         LIMIT %(param_1)s",
            "        OFFSET %(param_2)s",
            "       ) AS color",
            " ORDER BY color.count_1 ASC",
        ],
        "all-functions-hoisted-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               coalesce(anon_1.count_1, ?) AS coalesce_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         ORDER BY coalesce(anon_1.count_1, ?) ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            " ORDER BY color.coalesce_1 ASC",
        ],
        "all-functions-hoisted-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               coalesce(anon_1.count_1, %s) AS coalesce_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         ORDER BY coalesce(anon_1.count_1, %s) ASC",
            "         LIMIT %s,",
            "               %s",
            "       ) AS color",
            " ORDER BY color.coalesce_1 ASC",
        ],
        "selected-function-hoisted-postgresql": [
            "SELECT color.id,",
            "       color.count_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.count_1 AS count_1,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          JOIN LATERAL (",
            "                SELECT sum(fruit_1.sweetness) AS sum_1,",
            "                       count(*) AS count_1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_1",
            "            ON TRUE",
            "         ORDER BY anon_1.sum_1 ASC",
            "         LIMIT %(param_1)s",
            "        OFFSET %(param_2)s",
            "       ) AS color",
            " ORDER BY color.sum_1 ASC",
        ],
        "selected-function-hoisted-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT sum(fruit_1.sweetness) AS sum_1,",
            "               count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(color.count_1, ?) AS coalesce_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.count_1 AS count_1,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         ORDER BY anon_1.sum_1 ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            " ORDER BY color.sum_1 ASC",
        ],
        "selected-function-hoisted-mysql": [
            "WITH anon_1 AS (",
            "        SELECT sum(fruit_1.sweetness) AS sum_1,",
            "               count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(color.count_1, %s) AS coalesce_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.count_1 AS count_1,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         ORDER BY anon_1.sum_1 ASC",
            "         LIMIT %s,",
            "               %s",
            "       ) AS color",
            " ORDER BY color.sum_1 ASC",
        ],
        "filtered-function-hoisted-postgresql": [
            "SELECT color.id,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          JOIN LATERAL (",
            "                SELECT count(*) AS count_1,",
            "                       sum(fruit_1.sweetness) AS sum_1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_1",
            "            ON TRUE",
            "         WHERE anon_1.count_1 > %(param_1)s",
            "         ORDER BY color.id ASC",
            "         LIMIT %(param_2)s",
            "        OFFSET %(param_3)s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "filtered-function-hoisted-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         WHERE coalesce(anon_1.count_1, ?) > ?",
            "         ORDER BY color.id ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "filtered-function-hoisted-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         WHERE coalesce(anon_1.count_1, %s) > %s",
            "         ORDER BY color.id ASC",
            "         LIMIT %s,",
            "               %s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "unjoined-aggregation-not-hoisted-postgresql": [
            "SELECT color.id,",
            "       anon_1.count_1",
            "  FROM (",
            "        SELECT color.id AS id",
            "          FROM color AS color",
            "         ORDER BY color.id ASC",
            "         LIMIT %(param_1)s",
            "        OFFSET %(param_2)s",
            "       ) AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY color.id ASC",
        ],
        "unjoined-aggregation-not-hoisted-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, ?) AS coalesce_1",
            "  FROM (",
            "        SELECT color.id AS id",
            "          FROM color AS color",
            "         ORDER BY color.id ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY color.id ASC",
        ],
        "unjoined-aggregation-not-hoisted-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, %s) AS coalesce_1",
            "  FROM (",
            "        SELECT color.id AS id",
            "          FROM color AS color",
            "         ORDER BY color.id ASC",
            "         LIMIT %s,",
            "               %s",
            "       ) AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY color.id ASC",
        ],
        "relation-filter-postgresql": [
            "SELECT color.id",
            "  FROM (",
            "        SELECT color.id AS id",
            "          FROM color AS color",
            "         WHERE EXISTS (",
            "                SELECT 1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "                   AND fruit_1.sweetness > %(sweetness_1)s",
            "               )",
            "         ORDER BY color.id ASC",
            "         LIMIT %(param_1)s",
            "        OFFSET %(param_2)s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "relation-filter-sqlite": [
            "SELECT color.id",
            "  FROM (",
            "        SELECT color.id AS id",
            "          FROM color AS color",
            "         WHERE EXISTS (",
            "                SELECT 1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "                   AND fruit_1.sweetness > ?",
            "               )",
            "         ORDER BY color.id ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "relation-filter-mysql": [
            "SELECT color.id",
            "  FROM (",
            "        SELECT color.id AS id",
            "          FROM color AS color",
            "         WHERE EXISTS (",
            "                SELECT 1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "                   AND fruit_1.sweetness > %s",
            "               )",
            "         ORDER BY color.id ASC",
            "         LIMIT %s,",
            "               %s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "two-functions-filtered-and-selected-postgresql": [
            "SELECT color.id,",
            "       color.count_1,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.count_1 AS count_1,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          JOIN LATERAL (",
            "                SELECT count(*) AS count_1,",
            "                       sum(fruit_1.sweetness) AS sum_1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_1",
            "            ON TRUE",
            "         WHERE anon_1.count_1 > %(param_1)s",
            "           AND anon_1.sum_1 > %(param_2)s",
            "         ORDER BY color.id ASC",
            "         LIMIT %(param_3)s",
            "        OFFSET %(param_4)s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "two-functions-filtered-and-selected-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(color.count_1, ?) AS coalesce_1,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.count_1 AS count_1,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         WHERE coalesce(anon_1.count_1, ?) > ?",
            "           AND anon_1.sum_1 > ?",
            "         ORDER BY color.id ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "two-functions-filtered-and-selected-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(color.count_1, %s) AS coalesce_1,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.count_1 AS count_1,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         WHERE coalesce(anon_1.count_1, %s) > %s",
            "           AND anon_1.sum_1 > %s",
            "         ORDER BY color.id ASC",
            "         LIMIT %s,",
            "               %s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
    }
)

INNER_JOIN_SQL = snapshot(
    {
        "inner-join-rewrite-postgresql": [
            'SELECT "group".id,',
            "       color_1.name,",
            "       color_1.id AS id_1",
            '  FROM "group" AS "group"',
            "  JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            " WHERE color_1.name = %(name_1)s",
            ' ORDER BY "group".id ASC,',
            "          color_1.id ASC",
        ],
        "inner-join-rewrite-sqlite": [
            'SELECT "group".id,',
            "       color_1.name,",
            "       color_1.id AS id_1",
            '  FROM "group" AS "group"',
            "  JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            " WHERE color_1.name = ?",
            ' ORDER BY "group".id ASC,',
            "          color_1.id ASC",
        ],
        "inner-join-rewrite-mysql": [
            "SELECT `group`.id,",
            "       color_1.name,",
            "       color_1.id AS id_1",
            "  FROM `group` AS `group`",
            " INNER JOIN color AS color_1",
            "    ON color_1.id = `group`.color_id",
            " WHERE color_1.name = %s",
            " ORDER BY `group`.id ASC,",
            "          color_1.id ASC",
        ],
        "no-inner-join-rewrite-postgresql": [
            "SELECT color.id,",
            "       fruit_1.sweetness,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE color.name = %(name_1)s",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "no-inner-join-rewrite-sqlite": [
            "SELECT color.id,",
            "       fruit_1.sweetness,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE color.name = ?",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "no-inner-join-rewrite-mysql": [
            "SELECT color.id,",
            "       fruit_1.sweetness,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE color.name = %s",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "to-many-no-inner-join-rewrite-postgresql": [
            "SELECT color.id,",
            "       fruit_1.sweetness,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE EXISTS (",
            "        SELECT 1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "           AND fruit_2.sweetness > %(sweetness_1)s",
            "       )",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "to-many-no-inner-join-rewrite-sqlite": [
            "SELECT color.id,",
            "       fruit_1.sweetness,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE EXISTS (",
            "        SELECT 1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "           AND fruit_2.sweetness > ?",
            "       )",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
        "to-many-no-inner-join-rewrite-mysql": [
            "SELECT color.id,",
            "       fruit_1.sweetness,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE EXISTS (",
            "        SELECT 1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "           AND fruit_2.sweetness > %s",
            "       )",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ],
    }
)

SELECTED_FUNCTION_SQL = snapshot(
    {
        "filter-only-paginated-postgresql": [
            "SELECT color.id,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          JOIN LATERAL (",
            "                SELECT count(*) AS count_1,",
            "                       sum(fruit_1.sweetness) AS sum_1",
            "                  FROM fruit AS fruit_1",
            "                 WHERE color.id = fruit_1.color_id",
            "               ) AS anon_1",
            "            ON TRUE",
            "         WHERE anon_1.count_1 > %(param_1)s",
            "         ORDER BY color.id ASC",
            "         LIMIT %(param_2)s",
            "        OFFSET %(param_3)s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "filter-only-paginated-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         WHERE coalesce(anon_1.count_1, ?) > ?",
            "         ORDER BY color.id ASC",
            "         LIMIT ?",
            "        OFFSET ?",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "filter-only-paginated-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       color.sum_1",
            "  FROM (",
            "        SELECT color.id AS id,",
            "               anon_1.sum_1 AS sum_1",
            "          FROM color AS color",
            "          LEFT OUTER JOIN anon_1",
            "            ON color.id = anon_1.color_id",
            "         WHERE coalesce(anon_1.count_1, %s) > %s",
            "         ORDER BY color.id ASC",
            "         LIMIT %s,",
            "               %s",
            "       ) AS color",
            " ORDER BY color.id ASC",
        ],
        "filter-only-postgresql": [
            "SELECT color.id,",
            "       anon_1.sum_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " WHERE anon_1.count_1 > %(param_1)s",
            " ORDER BY color.id ASC",
        ],
        "filter-only-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       anon_1.sum_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, ?) > ?",
            " ORDER BY color.id ASC",
        ],
        "filter-only-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       anon_1.sum_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, %s) > %s",
            " ORDER BY color.id ASC",
        ],
        "order-by-only-postgresql": [
            "SELECT color.id,",
            "       anon_1.count_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT avg(fruit_1.sweetness) AS avg_1,",
            "               count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " ORDER BY anon_1.avg_1 ASC",
        ],
        "order-by-only-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_1.sweetness) AS avg_1,",
            "               count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, ?) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY anon_1.avg_1 ASC",
        ],
        "order-by-only-mysql": [
            "WITH anon_1 AS (",
            "        SELECT avg(fruit_1.sweetness) AS avg_1,",
            "               count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, %s) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " ORDER BY anon_1.avg_1 ASC",
        ],
        "filtered-and-selected-postgresql": [
            "SELECT color.id,",
            "       anon_1.count_1",
            "  FROM color AS color",
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " WHERE anon_1.count_1 > %(param_1)s",
            " ORDER BY color.id ASC",
        ],
        "filtered-and-selected-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, ?) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, ?) > ?",
            " ORDER BY color.id ASC",
        ],
        "filtered-and-selected-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT color.id,",
            "       coalesce(anon_1.count_1, %s) AS coalesce_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN anon_1",
            "    ON color.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, %s) > %s",
            " ORDER BY color.id ASC",
        ],
        "nested-filter-only-postgresql": [
            'SELECT "group".id,',
            "       color_1.id AS id_1,",
            "       color_1.id AS group__color__id,",
            "       anon_1.sum_1",
            '  FROM "group" AS "group"',
            "  JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            "  JOIN LATERAL (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1",
            "          FROM fruit AS fruit_1",
            "         WHERE color_1.id = fruit_1.color_id",
            "       ) AS anon_1",
            "    ON TRUE",
            " WHERE anon_1.count_1 > %(param_1)s",
            ' ORDER BY "group".id ASC,',
            "          color_1.id ASC",
        ],
        "nested-filter-only-sqlite": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            '       ) SELECT "group".id,',
            "       color_1.id AS id_1,",
            "       color_1.id AS group__color__id,",
            "       anon_1.sum_1",
            '  FROM "group" AS "group"',
            "  JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            "  LEFT OUTER JOIN anon_1",
            "    ON color_1.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, ?) > ?",
            ' ORDER BY "group".id ASC,',
            "          color_1.id ASC",
        ],
        "nested-filter-only-mysql": [
            "WITH anon_1 AS (",
            "        SELECT count(*) AS count_1,",
            "               sum(fruit_1.sweetness) AS sum_1,",
            "               fruit_1.color_id AS color_id",
            "          FROM fruit AS fruit_1",
            "         WHERE fruit_1.color_id IS NOT NULL",
            "         GROUP BY fruit_1.color_id",
            "       ) SELECT `group`.id,",
            "       color_1.id AS id_1,",
            "       color_1.id AS group__color__id,",
            "       anon_1.sum_1",
            "  FROM `group` AS `group`",
            " INNER JOIN color AS color_1",
            "    ON color_1.id = `group`.color_id",
            "  LEFT OUTER JOIN anon_1",
            "    ON color_1.id = anon_1.color_id",
            " WHERE coalesce(anon_1.count_1, %s) > %s",
            " ORDER BY `group`.id ASC,",
            "          color_1.id ASC",
        ],
    }
)


@pytest.mark.inline_snapshot
@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])
@pytest.mark.parametrize(
    "query",
    [
        pytest.param(
            """
            {
                colors(orderBy: { fruitsAggregate: { count: ASC } }) {
                    fruitsAggregate { count }
                }
            }
            """,
            id="output-order-by",
        ),
        pytest.param(
            """
            {
                colors(filter: { fruitsAggregate: { count: { predicate: { gt: 0 } } } }) {
                    fruitsAggregate { count }
                }
            }
            """,
            id="output-filter",
        ),
        pytest.param(
            """
            {
                colors(
                    filter: { fruitsAggregate: { count: { predicate: { gt: 0 } } } },
                    orderBy: { fruitsAggregate: { avg: { sweetness: ASC } } }
                ) {
                    fruits { id }
                }
            }
            """,
            id="filter-order-by",
        ),
        pytest.param(
            """
            {
                colors(
                    filter: { fruitsAggregate: { avg: { arguments: [sweetness] predicate: { gt: 0 } } } },
                    orderBy: { fruitsAggregate: { avg: { sweetness: ASC } } }
                ) {
                    fruits { id }
                }
            }
            """,
            id="filter-order-by-same-aggregation",
        ),
        pytest.param(
            """
            {
                colors {
                    fruitsAggregate {
                        max { sweetness name }
                    }
                }
            }
            """,
            id="output-multiple-aggregations",
        ),
        pytest.param(
            """
            {
                colors(
                    filter: {
                        fruitsAggregate: {
                            sum: { arguments: [sweetness], predicate: { gt: 0 } },
                            avg: { arguments: [sweetness], predicate: { gt: 0 } }
                        }
                    }
                ) {
                    fruits { id }
                }
            }
            """,
            id="filter-multiple-aggregations",
        ),
        pytest.param(
            """
            {
                colors(
                    orderBy: { fruitsAggregate: { sum: { sweetness: ASC }, avg: { sweetness: ASC } } }
                ) {
                    fruits { id }
                }
            }
            """,
            id="order-by-multiple-aggregations",
        ),
        pytest.param(
            """
            {
                colors(orderBy: { fruitsAggregate: { sum: { sweetness: ASC } } }) {
                    id
                    fruitsAggregate { count }
                }
            }
            """,
            id="order-by-and-selected-functions",
        ),
    ],
)
def test_aggregation_computation_is_reused(
    query: str, dialect_name: str, captured_statements: list[Select[Any]], request: pytest.FixtureRequest
) -> None:
    """A single query is emitted with the aggregation computation reused (no duplicate subquery).

    Filtering, ordering and selecting the same aggregation share one subquery, and ORDER BY
    carries only the functions ordered by. Verified DB-free by compiling the statement per dialect.
    """
    result = schema.execute_sync(query, context_value=DialectContext(dialect_name))  # ty: ignore[invalid-argument-type]

    assert not result.errors
    assert result.data

    assert len(captured_statements) == 1
    compiled = str(captured_statements[0].compile(dialect=SQLA_DIALECTS[dialect_name]))
    assert format_sql(compiled).splitlines() == AGGREGATION_SQL[request.node.callspec.id]


@pytest.mark.inline_snapshot
@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])
@pytest.mark.parametrize(
    ("query",),  # noqa: PT006
    [
        pytest.param(
            """
            {
                groups(filter: { color: { name: { eq: "x" } } }) {
                    color { name }
                }
            }
            """,
            id="inner-join-rewrite",
        ),
        pytest.param(
            """
            {
                colors(filter: { fruits: { sweetness: { gt: 1 } } }) {
                    fruits { sweetness }
                }
            }
            """,
            id="to-many-no-inner-join-rewrite",
        ),
        pytest.param(
            """
            {
                colors(filter: { name: { eq: "x" } }) {
                    fruits { sweetness }
                }
            }
            """,
            id="no-inner-join-rewrite",
        ),
    ],
)
def test_inner_join_rewriting(
    query: str, dialect_name: str, captured_statements: list[Select[Any]], request: pytest.FixtureRequest
) -> None:
    """A to-one filter on the null-supplying side rewrites the LEFT OUTER JOIN to INNER.

    A filter on the parent's own column, or on a to-many relation tested in EXISTS, keeps the outer join.
    """
    result = schema.execute_sync(query, context_value=DialectContext(dialect_name))  # ty: ignore[invalid-argument-type]

    assert not result.errors
    assert result.data
    assert len(captured_statements) == 1

    statement_str = str(captured_statements[0].compile(dialect=SQLA_DIALECTS[dialect_name]))
    assert format_sql(statement_str).splitlines() == INNER_JOIN_SQL[request.node.callspec.id]


@pytest.mark.inline_snapshot
@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])
@pytest.mark.parametrize(
    "query",
    [
        pytest.param(
            """
            {
                colorsPaginated(limit: 2, orderBy: { fruitsAggregate: { count: ASC } }) {
                    id
                }
            }
            """,
            id="all-functions-hoisted",
        ),
        pytest.param(
            """
            {
                colorsPaginated(limit: 2, orderBy: { fruitsAggregate: { sum: { sweetness: ASC } } }) {
                    id
                    fruitsAggregate { count }
                }
            }
            """,
            id="selected-function-hoisted",
        ),
        pytest.param(
            """
            {
                colorsPaginated(limit: 2, filter: { fruitsAggregate: { count: { predicate: { gt: 1 } } } }) {
                    id
                    fruitsAggregate { sum { sweetness } }
                }
            }
            """,
            id="filtered-function-hoisted",
        ),
        pytest.param(
            """
            {
                colorsPaginated(limit: 2, filter: { fruitsAggregate: {
                      count: { predicate: { gt: 1 } },
                      sum: { arguments: [sweetness], predicate: { gt: 0 } } } }) {
                    id
                    fruitsAggregate { count sum { sweetness } }
                }
            }
            """,
            id="two-functions-filtered-and-selected",
        ),
        pytest.param(
            """
            {
                colorsPaginated(limit: 2) {
                    id
                    fruitsAggregate { count }
                }
            }
            """,
            id="unjoined-aggregation-not-hoisted",
        ),
        pytest.param(
            """
            {
                colorsPaginated(limit: 2, filter: { fruits: { sweetness: { gt: 1 } } }) {
                    id
                }
            }
            """,
            id="relation-filter",
        ),
    ],
)
def test_pagination_subquery_outer_joins(
    query: str, dialect_name: str, captured_statements: list[Select[Any]], request: pytest.FixtureRequest
) -> None:
    """Outer joins hang off the pagination subquery, and an aggregation it already computed is not re-joined.

    An aggregation the subquery joins is hoisted whole and re-projected from it, leaving no outer
    join; one it does not join stays outside and is computed over the page. A join bound to the
    discarded inner alias instead of the subquery would surface as a second FROM element.
    """
    result = schema.execute_sync(query, context_value=DialectContext(dialect_name))  # ty: ignore[invalid-argument-type]

    assert not result.errors
    assert result.data
    assert len(captured_statements) == 1

    statement = captured_statements[0]
    assert len(statement.get_final_froms()) == 1
    compiled = str(statement.compile(dialect=SQLA_DIALECTS[dialect_name]))
    assert format_sql(compiled).splitlines() == PAGINATION_JOIN_SQL[request.node.callspec.id]


@pytest.mark.inline_snapshot
@pytest.mark.parametrize("dialect_name", ["postgresql", "sqlite", "mysql"])
@pytest.mark.parametrize(
    "query",
    [
        pytest.param(
            """
            {
                colorsPaginated(limit: 2, filter: { fruitsAggregate: { count: { predicate: { gt: 1 } } } }) {
                    id
                    fruitsAggregate { sum { sweetness } }
                }
            }
            """,
            id="filter-only-paginated",
        ),
        pytest.param(
            """
            {
                colors(filter: { fruitsAggregate: { count: { predicate: { gt: 1 } } } }) {
                    id
                    fruitsAggregate { sum { sweetness } }
                }
            }
            """,
            id="filter-only",
        ),
        pytest.param(
            """
            {
                colors(orderBy: { fruitsAggregate: { avg: { sweetness: ASC } } }) {
                    id
                    fruitsAggregate { count }
                }
            }
            """,
            id="order-by-only",
        ),
        pytest.param(
            """
            {
                colors(filter: { fruitsAggregate: { count: { predicate: { gt: 1 } } } }) {
                    id
                    fruitsAggregate { count }
                }
            }
            """,
            id="filtered-and-selected",
        ),
        pytest.param(
            """
            {
                groups(filter: { color: { fruitsAggregate: { count: { predicate: { gt: 1 } } } } }) {
                    id
                    color { id fruitsAggregate { sum { sweetness } } }
                }
            }
            """,
            id="nested-filter-only",
        ),
    ],
)
def test_projection_selects_only_requested_functions(
    query: str, dialect_name: str, captured_statements: list[Select[Any]], request: pytest.FixtureRequest
) -> None:
    """Only the aggregation functions the selection asks for reach the projection.

    A function a WHERE predicate or an ORDER BY term needs is still computed by the join and,
    under pagination, still exported by the subquery; it just is not shipped to the client. A
    function that is both filtered and selected is projected exactly once.
    """
    result = schema.execute_sync(query, context_value=DialectContext(dialect_name))  # ty: ignore[invalid-argument-type]

    assert not result.errors
    assert result.data
    assert len(captured_statements) == 1

    compiled = str(captured_statements[0].compile(dialect=SQLA_DIALECTS[dialect_name]))
    assert format_sql(compiled).splitlines() == SELECTED_FUNCTION_SQL[request.node.callspec.id]


@pytest.mark.inline_snapshot
def test_allowed_duplicate_hooked_relation(captured_statements: list[Select[Any]]) -> None:
    """A hooked to-one filtered and selected is joined twice: once without its hook, once with it."""
    query = '{ groupsVisibleColor(filter: { color: { name: { eq: "red" } } }) { name color { name } } }'

    result = schema.execute_sync(query, context_value=DialectContext("postgresql"))

    assert not result.errors
    assert len(captured_statements) == 1
    compiled = str(captured_statements[0].compile(dialect=SQLA_DIALECTS["postgresql"]))
    assert format_sql(compiled).splitlines() == snapshot(
        [
            'SELECT "group".name,',
            '       "group".id,',
            "       color_1.name AS name_1,",
            "       color_1.id AS id_1",
            '  FROM "group" AS "group"',
            "  JOIN color AS color_2",
            '    ON color_2.id = "group".color_id',
            "  LEFT OUTER JOIN color AS color_1",
            '    ON color_1.id = "group".color_id',
            "   AND color_1.name != %(name_2)s",
            " WHERE color_2.name = %(name_3)s",
            ' ORDER BY "group".id ASC,',
            "          color_1.id ASC",
        ]
    )


@pytest.mark.inline_snapshot
def test_allowed_duplicate_exists_and_selection(captured_statements: list[Select[Any]]) -> None:
    """A to-many filtered in an EXISTS and selected is read twice: the selection keeps every related row."""
    query = "{ colors(filter: { fruits: { sweetness: { gt: 5 } } }) { name fruits { name } } }"

    result = schema.execute_sync(query, context_value=DialectContext("postgresql"))

    assert not result.errors
    assert len(captured_statements) == 1
    compiled = str(captured_statements[0].compile(dialect=SQLA_DIALECTS["postgresql"]))
    assert format_sql(compiled).splitlines() == snapshot(
        [
            "SELECT color.name,",
            "       color.id,",
            "       fruit_1.name AS name_1,",
            "       fruit_1.id AS id_1",
            "  FROM color AS color",
            "  LEFT OUTER JOIN fruit AS fruit_1",
            "    ON color.id = fruit_1.color_id",
            " WHERE EXISTS (",
            "        SELECT 1",
            "          FROM fruit AS fruit_2",
            "         WHERE color.id = fruit_2.color_id",
            "           AND fruit_2.sweetness > %(sweetness_1)s",
            "       )",
            " ORDER BY color.id ASC,",
            "          fruit_1.id ASC",
        ]
    )
