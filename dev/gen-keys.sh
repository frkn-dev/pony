#!/bin/bash

DAYS=30
COUNT=20
SQL_FILE="fill_${DAYS}_days.sql"
SECRET="supetsecrettoken"


echo "Generating $COUNT keys $DAYS days..."
echo "INSERT INTO keys (key, status, duration_days) VALUES" > $SQL_FILE

for ((i=1; i<=COUNT; i++))
do
    RESP=$(curl -s -X POST http://localhost:5005/key \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${SECRET}" \
        -d "{\"days\": $DAYS, \"distributor\": \"FRKN\"}")

    CODE=$(echo $RESP | jq -r '.response.instance.Key.code')

    if [ "$CODE" != "null" ]; then
        SUFFIX=","
        [ $i -eq $COUNT ] && SUFFIX=";"
        echo "('$CODE', 'AVAILABLE', $DAYS)$SUFFIX" >> $SQL_FILE
        echo "[$i] Добавлен: $CODE"
    else
        echo "Error key generating $i"
    fi
done

echo "File $SQL_FILE is ready. "
