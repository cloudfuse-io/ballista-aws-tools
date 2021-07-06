import boto3
import glob

s3 = boto3.client('s3')


def lambda_handler(event, context):
    existing_files = glob.glob("/mnt/data/*")
    bucket = 'cloudfuse-taxi-data'
    for file_name in ['customer', 'lineitem', 'nation', 'orders', 'part', 'partsupp', 'region', 'supplier']:
        local_path = f'/mnt/data/{file_name}.tbl'
        key = f'tpch/tbl-s1/{file_name}.tbl'
        if local_path in existing_files:
            print(f'{local_path} already exists')
            continue
        try:
            s3.download_file(bucket, key, local_path)
        except Exception as e:
            print(e)
            print('Error getting object {} from bucket {}. Make sure they exist and your bucket is in the same region as this function.'.format(key, bucket))
            raise e
