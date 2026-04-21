ls -1 tests/tasks/*.tasks | parallel 'bash update-single-test.sh {}'
echo "updated"

