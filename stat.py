import binascii
import datetime
import io
import logging
import stat
import sys
import threading
from typing import IO, Text

from absl import app
from absl import flags
from fleetspeak.server_connector.connector import InsecureGRPCServiceClient
from fleetspeak.src.common.proto.fleetspeak.common_pb2 import Message

from stat_pb2 import Request, Response

FLAGS = flags.FLAGS

flags.DEFINE_string(
    name="client_id",
    default="",
    help="An id of the client to send the messages to.")

flags.DEFINE_string(
    name="output",
    default="",
    help="A path to the file to write the output to.")


def format_timestamp(hint, timestamp):
    return (
            hint + " {\n"
            f"  seconds: {timestamp.seconds}\n"
            f"  nanos: {timestamp.nanos}\n"
            f"  human readable: "
            f"\"{datetime.datetime.fromtimestamp(timestamp.seconds)}\""
            "\n}"
    )


def write(filedesc: IO[Text], response: Response):
    last_access = format_timestamp(
        'last access', response.extra.last_access_time)
    last_data_modification = format_timestamp(
        'last data modification', response.extra.last_data_modification_time)
    last_status_change = format_timestamp(
        'last status change', response.extra.last_status_change_time)

    if response.status.success:
        response_text = (
            f"path: {response.path}\n"
            f"size: {response.size} bytes\n"
            f"mode: {stat.filemode(response.mode)}\n"
            f"node: {response.extra.inode}\n"
            f"hardlinks number: {response.extra.hardlinks_number}\n"
            "owner {\n"
            f"  uid: {response.extra.owner.uid}\n"
            f"  name: \"{response.extra.owner.name}\"\n"
            "}\n"
            "owner group {\n"
            f"  gid: {response.extra.owner_group.gid}\n"
            f"  name: \"{response.extra.owner_group.name}\"\n"
            "}\n"
            f"{last_access}\n"
            f"{last_data_modification}\n"
            f"{last_status_change}\n\n"
        )
        filedesc.write(response_text)
    else:
        filedesc.write(f"stat on \"{response.path}\" failed:\n")
        filedesc.write(f"{response.status.error_details}\n\n")


def listener(message: Message, context):
    del context  # Unused

    response = Response()
    response.ParseFromString(message.data.value)

    if FLAGS.output:
        with io.open(FLAGS.output, mode="a", encoding="utf-8") as filedesc:
            write(filedesc, response)
    else:
        write(sys.stdout, response)


def main(argv=None):
    del argv  # Unused.

    service_client = InsecureGRPCServiceClient("stat")
    service_client.Listen(listener)

    while True:
        request = Request()
        request.path = input("Enter a path to stat: ")

        message = Message()
        message.destination.client_id = binascii.unhexlify(FLAGS.client_id)
        message.destination.service_name = "stater"
        message.data.Pack(request)

        service_client.Send(message)


if __name__ == "__main__":
    app.run(main)
