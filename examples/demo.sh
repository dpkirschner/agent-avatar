#!/bin/sh
# Synthetic demo: drive a local display server through the presets and
# card types. All data here is invented.
#
#   examples/demo.sh <scene>     scenes: chat split focus alert screen
#                                        calendar transcript clear
#
# B (server URL, default http://127.0.0.1:8075) and TOKEN (if the server
# has one) come from the environment.
set -eu
B=${B:-http://127.0.0.1:8075}

post() {
  if [ -n "${TOKEN:-}" ]; then
    curl -fsS -X POST -H 'Content-Type: application/json' \
      -H "Authorization: Bearer $TOKEN" -d "$2" "$B/$1" >/dev/null
  else
    curl -fsS -X POST -H 'Content-Type: application/json' \
      -d "$2" "$B/$1" >/dev/null
  fi
}

case "${1:-}" in
chat)
  post display '{"preset":"chat","content":[],"status":{"top":{"template":"desk","items":{"alerts":0,"mail":3,"next":"Team sync 10:30"}}}}'
  post say '{"text":"Morning. Three new emails, nothing urgent, and your team sync is at ten thirty."}'
  ;;
split)
  post display '{"preset":"split","content":[
    {"type":"figures","title":"This week","items":[
      {"label":"Steps","value":61240,"delta":"+8%","tone":"good"},
      {"label":"Sleep","value":7.2,"unit":"h","delta":"-0.3h","tone":"bad"},
      {"label":"Workouts","value":3}]},
    {"type":"goals","items":[
      {"name":"Run 40 km this month","progress":0.55,"current":22,"target":40,"due":"2026-11-30"},
      {"name":"Read 12 books","progress":0.83,"current":10,"target":12}]}]}'
  post say '{"text":"You are ahead on steps this week, a little short on sleep."}'
  ;;
focus)
  post display '{"preset":"focus","content":[
    {"type":"chart","title":"Groceries per week","kind":"bar","unit":"USD","series":[{"name":"Groceries","points":[
      ["2026-09-28",82],["2026-10-05",95],["2026-10-12",71],["2026-10-19",140],
      ["2026-10-26",88],["2026-11-02",92],["2026-11-09",79]]}],
     "highlight":{"at":"2026-10-19"}},
    {"type":"list","title":"That week","items":[
      {"text":"Party supplies","detail":"one-off"},
      {"text":"Usual shop"}]}]}'
  post say '{"text":"The spike was the week of October nineteenth: party supplies."}'
  ;;
alert)
  post display '{"content":[{"type":"alerts","items":[
    {"level":"crit","title":"Backup disk is full","detail":"Nightly backup failed at 02:00.","since":"2026-11-14T02:00"},
    {"level":"warn","title":"Certificate renews in 5 days"}]}],
    "status":{"top":{"template":"desk","items":{"alerts":2,"mail":3}}}}'
  post say '{"text":"Heads up: the backup disk is full and last night'\''s backup failed."}'
  ;;
screen)
  post display '{"preset":"focus","content":[{"type":"screen","name":"weather","params":{"range":"7d"}}]}'
  ;;
calendar)
  post display '{"preset":"split","content":[{"type":"calendar","events":[
    {"start":"2026-11-14","title":"Farmers market","all_day":true},
    {"start":"2026-11-14T10:30","end":"2026-11-14T11:00","title":"Team sync","where":"Video call"},
    {"start":"2026-11-14T15:00","title":"Dentist","where":"Main St clinic"},
    {"start":"2026-11-15T09:00","title":"Long run"}]},
    {"type":"text","title":"Note","body":"Bring the parking ticket to the dentist."}]}'
  ;;
transcript)
  post display '{"preset":"split","content":[{"type":"transcript","lines":20}],
    "status":{"bottom":{"template":"note","items":{"text":"Ask me to show anything from finance or health."}}}}'
  ;;
clear)
  post display '{"content":[],"status":{"top":null,"bottom":null},"avatar":"idle"}'
  ;;
*)
  sed -n '2,10p' "$0"
  exit 2
  ;;
esac
