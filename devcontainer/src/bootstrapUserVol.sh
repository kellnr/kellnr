#!/bin/bash
/bin/bash -c "tar -C /etc -cf - . | tar -C /tmp/etc -xf -" > /dev/null
