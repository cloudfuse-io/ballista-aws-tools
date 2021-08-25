import boto3
import glob
import gzip
import os

s3 = boto3.client('s3')

TPCH_TABLE_NAMES = ['customer', 'lineitem', 'nation',
                    'orders', 'part', 'partsupp', 'region', 'supplier']


def check_region(region):
    if os.environ['AWS_REGION'] != region:
        raise Exception(
            f"Your stack is in {os.environ['AWS_REGION']} but the bucket is in {region}")


def copy_cf():
    existing_files = glob.glob("/mnt/data/*")
    print('existing_files:', existing_files)
    bucket = 'cloudfuse-taxi-data'
    for file_name in TPCH_TABLE_NAMES:
        local_path = f'/mnt/data/{file_name}.tbl'
        key = f'tpch/tbl-s1/{file_name}.tbl'
        if local_path in existing_files:
            print(f'{local_path} already exists')
            continue
        try:
            s3.download_file(bucket, key, local_path)
        except Exception as e:
            print(e)
            print(
                f'Error getting object {key} from bucket {bucket}. Make sure they exist and your bucket is in the same region as this function.')
            raise e


def copy_memsql():
    existing_files = glob.glob("/mnt/data/**", recursive=True)
    print('existing_files:', existing_files)
    bucket = 'memsql-tpch-dataset'
    for table_name in TPCH_TABLE_NAMES:
        s3ls_res = s3.list_objects_v2(
            Bucket=bucket,
            Prefix=f'sf_100/{table_name}',
        )
        for (i, ls_key) in enumerate(s3ls_res['Contents']):
            key = ls_key['Key']
            if key.endswith('/'):
                continue
            partition_name = f'{table_name}/{i:02d}.tbl'
            tmp_local_path = f'/mnt/data/tmp/{partition_name}'
            local_path = f'/mnt/data/{partition_name}'
            if local_path in existing_files:
                print(f'{local_path} already exists')
                continue
            try:
                print(f'starting dl of: {key}')
                obj = s3.get_object(
                    Bucket=bucket,
                    Key=key,
                )
                os.makedirs(os.path.dirname(tmp_local_path), exist_ok=True)
                with open(tmp_local_path, 'wb') as f:
                    with gzip.GzipFile(fileobj=obj["Body"]) as gzipfile:
                        for chunk in gzipfile:
                            f.write(chunk)
                os.makedirs(os.path.dirname(local_path), exist_ok=True)
                os.rename(tmp_local_path, local_path)
                print(f'{key} downloaded as {local_path}')
            except Exception as e:
                print(e)
                print(
                    f'Error getting object {key} from bucket {bucket}. Make sure they exist and your bucket is in the same region as this function.')
                raise e


def lambda_handler(event, context):
    bucket = event.get('bucket', 'memsql-tpch-dataset')
    mode = event.get('mode', 'copy')
    if mode == 'copy':
        if bucket == 'memsql-tpch-dataset':
            check_region('us-east-1')
            copy_memsql()
        elif bucket == 'cloudfuse-taxi-data':
            check_region('us-east-2')
            copy_cf()
        else:
            raise Exception(f'Unknown bucket: {bucket}')
    elif mode == 'delete':
        for filename in glob.glob("/mnt/data/**/*"):
            try:
                os.remove(filename)
                print('deleting', filename)
            except:
                print('skiping', filename)
    elif mode == 'list':
        for filename in glob.glob("/mnt/data/**/*"):
            print(os.stat(filename).st_size, '-->', filename)  

