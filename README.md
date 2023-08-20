# Ferrite Queue

This is a proof-of-concept for a simple task queue. It can ingest work through
HTTP requests, which are then sent to workers through websockets. At-least-once
delivery is guaranteed.

## Known Limitations

- Error handling is not robust, server threads may crash unexpectedly. This
  *probably* won't cause data loss, but might cause weird behavior.
- A worker pausing and not ack'ing the request will stall the entire queue, even
  if there are other workers that could pick up the work.

As a reminder, this is just a proof of concept and should not be used for
anything other than as an example.
